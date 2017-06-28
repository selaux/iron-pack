use iron::prelude::*;
use iron::headers::*;
use iron::modifier::Modifier;
use iron::response::WriteBody;

/// The trait that needs to be implemented to compress the response body when the client
/// sends a specific header
///
/// # Example
///
/// An identiy modifier
///
/// ```rust,no_run
/// use iron_pack::{ Encoding, CompressionModifier, WriteBody };
///
/// struct IdentityModifier {}
///
/// impl CompressionModifier for IdentityModifier {
///     fn get_header(&self) -> Encoding { Encoding::EncodingExt(String::from("br")) }
///
///     fn compress_body(&self, body: &mut Box<WriteBody>) -> Result<Vec<u8>, String> {
///         let mut data: Vec<u8> = Vec::new();
///         body.write_body(&mut data).map_err(|e| format!("{}", e))?;
///         return Ok(data)
///     }
/// }
///
/// ```
pub trait CompressionModifier {
    /// Returns the encoding header the compression modifier should respond to
    fn get_header(&self) -> Encoding;
    /// Returns the compressed request body
    fn compress_body(&self, res: &mut Box<WriteBody>) -> Result<Vec<u8>, String>;
}

impl PartialEq for CompressionModifier {
    fn eq(&self, other: &CompressionModifier) -> bool { self.get_header() == other.get_header() }
    fn ne(&self, other: &CompressionModifier) -> bool { self.get_header() != other.get_header() }
}

impl<'a> Modifier<Response> for &'a CompressionModifier {
    fn modify(self, mut res: &mut Response) {
        let encoded = match res.body {
            Some(ref mut body) => self.compress_body(body),
            None => return ()
        };

        match encoded {
            Ok(compressed_bytes) => {
                res.headers.set(ContentEncoding(vec![self.get_header()]));
                compressed_bytes.modify(res);
            },
            Err(_) => {}
        };
    }
}