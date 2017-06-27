extern crate iron;
extern crate iron_pack;

use iron::prelude::*;
use iron_pack::CompressionMiddleware;

fn a_lot_of_batman(_: &mut Request) -> IronResult<Response> {
    let nana = "Na".repeat(5000);
    Ok(Response::with((iron::status::Ok, format!("{}, Batman!", nana))))
}

fn main() {
    let mut chain = Chain::new(a_lot_of_batman);
    chain.link_after(CompressionMiddleware);
    Iron::new(chain).http("0.0.0.0:3000").unwrap();
}