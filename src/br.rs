use std::io;
use iron::prelude::*;
use iron::headers::*;
use brotli::CompressorWriter;
use writer::ContentEncoding;

fn stringify_err(err: io::Error) -> String { format!("Error compressing body: {}", err) }

const BUFFER_SIZE: usize = 4096;
const QUALITY: u32 = 8;
const LG_WINDOW_SIZE: u32 = 20;

pub struct Brotli;

impl ContentEncoding for Brotli {
    fn get_header(&self) -> Encoding {
        Encoding::EncodingExt(String::from("br"))
    }

    fn compress_body(&self, res: &mut Response) -> Result<Vec<u8>, String> {
        if let Some(ref mut body) = res.body {
            let mut data: Vec<u8> = Vec::new();
            {
                let mut encoder = CompressorWriter::new(&mut data, BUFFER_SIZE, QUALITY, LG_WINDOW_SIZE);
                body.write_body(&mut encoder).map_err(stringify_err)?;
            }
            return Ok(data);
        } else {
            Err(String::from("Error compressing body: No response body present."))
        }
    }
}