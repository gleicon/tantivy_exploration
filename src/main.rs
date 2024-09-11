use error_chain::error_chain;
use glob::{glob_with, MatchOptions};
use std::fs;
use std::path::PathBuf;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};
use tempfile::TempDir;

error_chain! {
    foreign_links {
        Glob(glob::GlobError);
        Pattern(glob::PatternError);
    }
}

fn main() -> tantivy::Result<()> {
    let index_path = "nom_index";

    fs::create_dir_all(index_path)?;
    let index_directory = tantivy::directory::MmapDirectory::open(index_path)?;

    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT | STORED);
    let schema = schema_builder.build();
    let index = Index::open_or_create(index_directory, schema.clone())?;

    let mut index_writer: IndexWriter = index.writer(50_000_000)?;

    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();

    let _ = index_all_pdfs(title, body, &mut index_writer);

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()?;

    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![title, body]);

    let query = query_parser.parse_query("linux")?;

    let top_docs = searcher.search(&query, &TopDocs::with_limit(10))?;
    for (_score, doc_address) in top_docs {
        let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
        println!("{}", retrieved_doc.to_json(&schema));
    }

    //let query = query_parser.parse_query("title:sea^20 body:whale^70")?;

    // let (_score, doc_address) = searcher
    //     .search(&query, &TopDocs::with_limit(1))?
    //     .into_iter()
    //     .next()
    //     .unwrap();

    // let explanation = query.explain(&searcher, doc_address)?;

    // println!("{}", explanation.to_pretty_json());

    Ok(())
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
                        //return Ok("Parsed".into());
                    }
                    Err(e) => {
                        println!("Error Parsing {:?}", e);
                        //return Err(e);
                    }
                }
                // parse
            }
            Err(_e) => {
                //println!("Error {}", e);
                ()
            }
        }
    }

    return Ok("Done".into());
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

            //println!("Text on page {}: {}", 42, texts[41]);
            Ok(texts)
        }
        Err(e) => {
            println!("Error: {}", e);
            return Err("erro".into());
        }
    }
}
