//! Tonic codec over prost-reflect DynamicMessage, so RPCs discovered at
//! runtime via reflection can be invoked without compiled-in stubs.

use prost::Message;
use prost_reflect::{DynamicMessage, MessageDescriptor, MethodDescriptor};
use tonic::codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder};
use tonic::Status;

#[derive(Clone)]
pub struct DynamicCodec {
    method: MethodDescriptor,
}

impl DynamicCodec {
    pub fn new(method: MethodDescriptor) -> Self {
        Self { method }
    }
}

impl Codec for DynamicCodec {
    type Encode = DynamicMessage;
    type Decode = DynamicMessage;
    type Encoder = DynamicEncoder;
    type Decoder = DynamicDecoder;

    fn encoder(&mut self) -> Self::Encoder {
        DynamicEncoder
    }

    fn decoder(&mut self) -> Self::Decoder {
        DynamicDecoder(self.method.output())
    }
}

pub struct DynamicEncoder;

impl Encoder for DynamicEncoder {
    type Item = DynamicMessage;
    type Error = Status;

    fn encode(&mut self, item: Self::Item, dst: &mut EncodeBuf<'_>) -> Result<(), Status> {
        item.encode(dst)
            .map_err(|e| Status::internal(format!("encode: {e}")))
    }
}

pub struct DynamicDecoder(MessageDescriptor);

impl Decoder for DynamicDecoder {
    type Item = DynamicMessage;
    type Error = Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Status> {
        let msg = DynamicMessage::decode(self.0.clone(), src)
            .map_err(|e| Status::internal(format!("decode: {e}")))?;
        Ok(Some(msg))
    }
}
