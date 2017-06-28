use std::io;
use iron::headers::*;
use iron::response::WriteBody;
use libflate::gzip;
use writer::ContentEncoding;

fn stringify_err(err: io::Error) -> String { format!("Error compressing body: {}", err) }

pub struct GZip;

impl ContentEncoding for GZip {
    fn get_header(&self) -> Encoding { Encoding::Gzip }

    fn compress_body(&self, body: &mut Box<WriteBody>) -> Result<Vec<u8>, String> {
        let mut encoder = gzip::Encoder::new(Vec::new()).map_err(stringify_err)?;
        body.write_body(&mut encoder).map_err(stringify_err)?;
        return encoder.finish().into_result().map_err(stringify_err);
    }
}