use iron::prelude::*;
use iron::headers::*;
use iron::modifier::Modifier;
use iron::response::WriteBody;

pub trait ContentEncoding {
    fn get_header(&self) -> Encoding;
    fn compress_body(&self, res: &mut Box<WriteBody>) -> Result<Vec<u8>, String>;
}

impl PartialEq for ContentEncoding {
    fn eq(&self, other: &ContentEncoding) -> bool {
        self.get_header() == other.get_header()
    }

    fn ne(&self, other: &ContentEncoding) -> bool {
        self.get_header() != other.get_header()
    }
}

impl<'a> Modifier<Response> for &'a ContentEncoding {
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