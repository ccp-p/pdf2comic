use std::path::PathBuf;

use lopdf::Document;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = PathBuf::from(&args[1]);
    let doc = Document::load(&path).unwrap();
    let pages: Vec<_> = doc.get_pages().values().copied().collect();
    println!("pages={}", pages.len());

    let start: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(0);
    let end: usize = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(3);
    for page_id in pages.iter().skip(start).take(end - start) {
        let dict = doc.get_dictionary(*page_id).unwrap();
        let resources = dict.get(b"Resources").unwrap().clone();
        let resolved = match &resources {
            lopdf::Object::Reference(id) => doc.get_object(*id).unwrap().clone(),
            o => o.clone(),
        };
        let res_dict = match &resolved {
            lopdf::Object::Dictionary(d) => d,
            _ => continue,
        };
        let xobjects = res_dict.get(b"XObject").unwrap().clone();
        let xobj_dict = match &xobjects {
            lopdf::Object::Reference(id) => match doc.get_object(*id).unwrap() {
                lopdf::Object::Dictionary(d) => d.clone(),
                _ => continue,
            },
            lopdf::Object::Dictionary(d) => d.clone(),
            _ => continue,
        };
        for (name, value) in xobj_dict.iter() {
            let obj = doc.get_object(value.as_reference().unwrap()).unwrap();
            if let lopdf::Object::Stream(s) = obj {
                if s.dict.get(b"Subtype").unwrap().as_name().unwrap() == b"Image" {
                    println!("--- page {:?} image {} ---", page_id, String::from_utf8_lossy(name));
                    println!("dict: {:#?}", s.dict);
                    println!("raw_len={}", s.content.len());
                }
            }
        }
    }
}
