use std::io;
use iron::headers::*;
use iron::response::WriteBody;
use libflate::deflate;
use compression_modifier::CompressionModifier;

fn stringify_err(err: io::Error) -> String { format!("Error compressing body: {}", err) }

pub struct Deflate;

impl CompressionModifier for Deflate {
    fn get_header(&self) -> Encoding { Encoding::Deflate }

    fn compress_body(&self, body: &mut Box<WriteBody>) -> Result<Vec<u8>, String> {
        let mut encoder = deflate::Encoder::new(Vec::new());
        body.write_body(&mut encoder).map_err(stringify_err)?;
        return encoder.finish().into_result().map_err(stringify_err);
    }
}