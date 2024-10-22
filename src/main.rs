use actix_web::{web, App, HttpServer};
use error_chain::error_chain;
use glob::{glob_with, MatchOptions};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use tantivy::query::QueryParser;
use tantivy::schema::*;

mod autosuggest;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};

error_chain! {
    foreign_links {
        Glob(glob::GlobError);
        Pattern(glob::PatternError);
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let index_path = PathBuf::from("./tantivy_pdf_index");
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT | STORED);
    let schema = schema_builder.build();

    // Check if the index already exists
    let index = if Path::new(&index_path).exists() {
        println!("Index already exists, opening existing index");
        Index::open_in_dir(&index_path).unwrap()
    } else {
        println!("Index does not exist, creating new index");
        fs::create_dir(index_path.clone())?;
        Index::create_in_dir(&index_path, schema.clone()).unwrap()
    };

    let mut index_writer: IndexWriter = index.writer(50_000_000).unwrap();

    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();
    // Check if the index is empty
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();

    if searcher.num_docs() == 0 {
        println!("Index is empty. Indexing PDFs...");
        let _ = index_all_pdfs(title, body, &mut index_writer);
        index_writer.commit().unwrap();
    } else {
        println!("Index already contains {} documents", searcher.num_docs());
    }

    // Only index PDFs if the index is newly created

    println!("Search is ready\ntry curl https://localhost:8080/autosuggest?prefix=test\n");
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()
        .unwrap();

    let _searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![title, body]);

    let app_state = web::Data::new(crate::autosuggest::AppState {
        index,
        query_parser,
    });

    HttpServer::new(move || {
        App::new().app_data(app_state.clone()).route(
            "/autosuggest",
            web::get().to(crate::autosuggest::autosuggest),
        )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

fn index_all_pdfs(
    title: tantivy::schema::Field,
    body: tantivy::schema::Field,
    index_writer: &mut tantivy::IndexWriter,
) -> Result<String> {
    let options = MatchOptions {
        case_sensitive: false,
        ..Default::default()
    };

    for entry in glob_with("/Users/gleicon/Downloads/*.pdf", options)? {
        match entry {
            Ok(pb) => {
                println!("parsing {}", pb.clone().display());
                match parse_pdf(pb.clone()) {
                    Ok(out) => {
                        //println!("out: {:?}", out);
                        let mut le_doc = TantivyDocument::default();
                        le_doc.add_text(title, pb.clone().display());

                        le_doc.add_text(body, out.join(" "));
                        let _ = index_writer.add_document(le_doc);

                        let _ = index_writer.commit();
                    }
                    Err(e) => {
                        println!("Error Parsing {:?}", e);
                    }
                }
                // parse
            }
            Err(e) => {
                //println!("Error {:?}", e);
                //()
            }
        }
    }

    Ok("Done".into())
}

fn parse_pdf(file: PathBuf) -> Result<Vec<String>> {
    match lopdf::Document::load(file.clone()) {
        Ok(document) => {
            let pages = document.get_pages();
            let mut texts = Vec::new();

            for (i, _) in pages.iter().enumerate() {
                let page_number = (i + 1) as u32;
                let text = document.extract_text(&[page_number]);
                texts.push(text.unwrap_or_default());
            }

            Ok(texts)
        }
        Err(e) => {
            println!("Error: {}", e);
            Err("erro".into())
        }
    }
}
