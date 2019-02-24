use crate::UidDict;

use reqwest;
use std::fs::read_to_string;
use sxd_document::parser::parse;
use sxd_xpath::{Context, Factory};

type DynResult<T> = Result<T, Box<dyn std::error::Error>>;

pub fn run(args: UidDict) -> DynResult<()> {
    let src = args.from;

    let txt = if src.starts_with("http:") || src.starts_with("https:") {
        reqwest::get(&src)?.text()?
    } else {
        read_to_string(src)?
    };

    let pkg = parse(&txt)?;

    let doc = pkg.as_document();

    let factory = Factory::new();

    // we want a `chapter` element of id `chapter_A`, then
    // iterate on `/table/tbody/tr`
    let xpath = factory
        .build("/d:book/d:chapter[@xml:id=\"chapter_A\"]/d:table/d:tbody/d:tr")
        .expect("could not compile XPath");
    let xpath = xpath.expect("No XPath was compiled");

    let mut context = Context::new();
    context.set_namespace("d", "http://docbook.org/ns/docbook");
    context.set_namespace("xl", "http://www.w3.org/1999/xlink");
    context.set_namespace("xml", "https://www.w3.org/XML/1998/namespace");

    let value = xpath
        .evaluate(&context, doc.root())
        .expect("XPath evaluation failed");

    Ok(())
}
