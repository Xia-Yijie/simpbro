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
                    simpbro::browser::PageLine::Text(t, _) => {
                        println!("     {}", t);
                    }
                    simpbro::browser::PageLine::LinkRef(t, idx, _) => {
                        println!("[Link#{}] {} -> {}", idx, t, page.links[*idx].url);
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
