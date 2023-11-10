use std::io;

use mio::{Evented, Poll, PollOpt, Ready, Token};
use futures::{
  pin_mut,
  stream::{FusedStream, Stream, StreamExt},
};
use rustdds::{
  dds::{ReadError, ReadResult, WriteResult},
  *,
};
use serde::{de::DeserializeOwned, Serialize};

use super::message_info::MessageInfo;

/// A ROS2 Publisher
///
/// Corresponds to a simplified [`DataWriter`](rustdds::no_key::DataWriter)in
/// DDS
pub struct Publisher<M: Serialize> {
  datawriter: no_key::DataWriterCdr<M>,
}

impl<M: Serialize> Publisher<M> {
  // These must be created from Node
  pub(crate) fn new(datawriter: no_key::DataWriterCdr<M>) -> Publisher<M> {
    Publisher { datawriter }
  }

  pub fn publish(&self, message: M) -> WriteResult<(), M> {
    self.datawriter.write(message, Some(Timestamp::now()))
  }

  // pub(crate) fn publish_with_options(
  //   &self,
  //   message: M,
  //   wo: WriteOptions,
  // ) -> dds::Result<rustdds::rpc::SampleIdentity> {
  //   self.datawriter.write_with_options(message, wo)
  // }

  pub fn assert_liveliness(&self) -> WriteResult<(), ()> {
    self.datawriter.assert_liveliness()
  }

  pub fn guid(&self) -> rustdds::GUID {
    self.datawriter.guid()
  }

  pub async fn async_publish(&self, message: M) -> WriteResult<(), M> {
    self
      .datawriter
      .async_write(message, Some(Timestamp::now()))
      .await
  }

  #[allow(dead_code)] // This is for async Service implementation. Remove this when it is implemented.
  pub(crate) async fn async_publish_with_options(
    &self,
    message: M,
    wo: WriteOptions,
  ) -> dds::WriteResult<rustdds::rpc::SampleIdentity, M> {
    self.datawriter.async_write_with_options(message, wo).await
  }
}
// ----------------------------------------------------
// ----------------------------------------------------
// ----------------------------------------------------
// ----------------------------------------------------
// ----------------------------------------------------

/// A ROS2 Subscription
///
/// Corresponds to a (simplified) [`DataReader`](rustdds::no_key::DataReader) in
/// DDS
pub struct Subscription<M: DeserializeOwned> {
  datareader: no_key::SimpleDataReaderCdr<M>,
}

impl<M: 'static + DeserializeOwned> Subscription<M> {
  // These must be created from Node
  pub(crate) fn new(datareader: no_key::SimpleDataReaderCdr<M>) -> Subscription<M> {
    Subscription { datareader }
  }

  pub fn take(&self) -> ReadResult<Option<(M, MessageInfo)>> {
    self.datareader.drain_read_notifications();
    let ds: Option<no_key::DeserializedCacheChange<M>> = self.datareader.try_take_one()?;
    Ok(ds.map(dcc_to_value_and_messageinfo))
  }

  pub async fn async_take(&self) -> ReadResult<(M, MessageInfo)> {
    let async_stream = self.datareader.as_async_stream();
    pin_mut!(async_stream);
    match async_stream.next().await {
      Some(Err(e)) => Err(e),
      Some(Ok(ds)) => Ok(dcc_to_value_and_messageinfo(ds)),
      // Stream from SimpleDataReader is not supposed to ever end.
      None => {
        read_error_internal!("async_take(): SimpleDataReader value stream unexpectedly ended!")
      }
    }
  }

  // Returns an async Stream of messages with MessageInfo metadata
  pub fn async_stream(
    &self,
  ) -> impl Stream<Item = ReadResult<(M, MessageInfo)>> + FusedStream + '_ {
    self
      .datareader
      .as_async_stream()
      .map(|result| result.map(dcc_to_value_and_messageinfo))
  }

  pub fn guid(&self) -> rustdds::GUID {
    self.datareader.guid()
  }
}

// helper
#[inline]
fn dcc_to_value_and_messageinfo<M>(dcc: no_key::DeserializedCacheChange<M>) -> (M, MessageInfo)
where
  M: DeserializeOwned,
{
  let mi = MessageInfo::from(&dcc);
  (dcc.into_value(), mi)
}

impl<D> Evented for Subscription<D>
where
  D: DeserializeOwned,
{
  // We just delegate all the operations to datareader, since it
  // already implements Evented
  fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
    self.datareader.register(poll, token, interest, opts)
  }

  fn reregister(
    &self,
    poll: &Poll,
    token: Token,
    interest: Ready,
    opts: PollOpt,
  ) -> io::Result<()> {
    self.datareader.reregister(poll, token, interest, opts)
  }

  fn deregister(&self, poll: &Poll) -> io::Result<()> {
    self.datareader.deregister(poll)
  }
}
