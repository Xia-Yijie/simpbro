use simpbro::browser::Browser;

fn main() {
    let mut browser = Browser::new().expect("Failed to create browser");

    let url = std::env::args().nth(1).unwrap_or_else(|| "https://example.com".to_string());
    println!("=== Fetching {} ===\n", url);
    match browser.fetch(&url) {
        Ok(page) => {
            println!("URL:   {}", page.url);
            println!("Title: {}", page.title);
            println!("Links: {}", page.links.len());
            println!("\n--- Content ---");
            for line in &page.lines {
                match line {
                    simpbro::browser::PageLine::Heading(t, level, _) => {
                        println!("[H{}] {}", level, t);
                    }
                    simpbro::browser::PageLine::Text(segs) => {
                        let s: String = segs.iter().map(|seg| {
                            if let Some(i) = seg.link_idx {
                                format!("[{}#{}]", seg.text, i)
                            } else {
                                seg.text.clone()
                            }
                        }).collect();
                        println!("     {}", s);
                    }
                    simpbro::browser::PageLine::ButtonRef(label, _, _) => {
                        println!("[Button] [{}]", label);
                    }
                    simpbro::browser::PageLine::InputRef(placeholder, idx, _) => {
                        let val = &page.inputs[*idx].value;
                        if val.is_empty() {
                            println!("[Input#{}] [{}]", idx, placeholder);
                        } else {
                            println!("[Input#{}] [{}] = \"{}\"", idx, placeholder, val);
                        }
                    }
                    simpbro::browser::PageLine::Blank => {
                        println!();
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
