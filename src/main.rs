use error_chain::error_chain;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};
use tempfile::TempDir;

fn main() -> tantivy::Result<()> {
    let index_path = TempDir::new()?;
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT);
    let schema = schema_builder.build();
    let index = Index::create_in_dir(&index_path, schema.clone())?;
    let mut index_writer: IndexWriter = index.writer(50_000_000)?;

    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();

    let _ = index_all_pdfs(title, body, &mut index_writer);

    let mut old_man_doc = TantivyDocument::default();
    old_man_doc.add_text(title, "The Old Man and the Sea");
    old_man_doc.add_text(
        body,
        "He was an old man who fished alone in a skiff in the Gulf Stream and he had gone \
         eighty-four days now without taking a fish.",
    );
    index_writer.add_document(old_man_doc)?;

    index_writer.add_document(doc!(
    title => "Of Mice and Men",
    body => "A few miles south of Soledad, the Salinas River drops in close to the hillside \
            bank and runs deep and green. The water is warm too, for it has slipped twinkling \
            over the yellow sands in the sunlight before reaching the narrow pool. On one \
            side of the river the golden foothill slopes curve up to the strong and rocky \
            Gabilan Mountains, but on the valley side the water is lined with trees—willows \
            fresh and green with every spring, carrying in their lower leaf junctures the \
            debris of the winter’s flooding; and sycamores with mottled, white, recumbent \
            limbs and branches that arch over the pool"
    ))?;

    index_writer.add_document(doc!(
    title => "Frankenstein",
    title => "The Modern Prometheus",
    body => "You will rejoice to hear that no disaster has accompanied the commencement of an \
             enterprise which you have regarded with such evil forebodings.  I arrived here \
             yesterday, and my first task is to assure my dear sister of my welfare and \
             increasing confidence in the success of my undertaking."
    ))?;

    index_writer.commit()?;

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()?;

    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![title, body]);

    let query = query_parser.parse_query("sea whale")?;

    let top_docs = searcher.search(&query, &TopDocs::with_limit(10))?;
    for (_score, doc_address) in top_docs {
        let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
        println!("{}", retrieved_doc.to_json(&schema));
    }

    //let query = query_parser.parse_query("title:sea^20 body:whale^70")?;

    //let (_score, doc_address) = searcher
    //    .search(&query, &TopDocs::with_limit(1))?
    //    .into_iter()
    //   .next()
    // .unwrap();

    //let explanation = query.explain(&searcher, doc_address)?;

    //println!("{}", explanation.to_pretty_json());

    Ok(())
}

fn index_all_pdfs(
    title: tantivy::schema::Field,
    body: tantivy::schema::Field,
    index_writer: &mut tantivy::IndexWriter,
) -> io::Result<()> {
    //let current_dir = env::current_dir()?;

    for entry in fs::read_dir("/Users/gleicon/Downloads")? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        //  let last_modified = metadata.modified()?.elapsed()?.as_secs();
        //println!("{:?}", path);

        // last_modified < 24 * 3600 &&  to index recent files
        // if is a file andends with pdf:w

        if metadata.is_file() {
            match path.extension() {
                Some(p) => {
                    let fname = path.file_name().ok_or(());
                    if p.eq_ignore_ascii_case("pdf") {
                        println!(
                            " is read only: {:?}, size: {:?} bytes, filename: {:?}",
                            metadata.permissions().readonly(),
                            metadata.len(),
                            fname
                        );

                        match parse_pdf(path.clone()) {
                            Ok(out) => {
                                let mut le_doc = TantivyDocument::default();
                                le_doc.add_text(title, fname?.to_str().unwrap_or(""));
                                le_doc.add_text(body, out.join(" "));
                                let _ = index_writer.add_document(le_doc);

                                let _ = index_writer.commit();
                                // return Ok(());

                                //println!("{:?}", out[0..10].to_string())
                            }
                            Err(e) => {
                                println!("Error {:?}", e);
                                return Err(e);
                            }
                        }
                        //let out = pdf_extract::extract_text_from_mem(&bytes);
                    }
                    //   Ok(())
                }
                None => (),
            }
        }
        return Ok(());
    }

    fn parse_pdf(file: PathBuf) -> Result<Vec<String>, std::io::Error> {
        match lopdf::Document::load(file) {
            Ok(document) => {
                let pages = document.get_pages();
                let mut texts = Vec::new();

                for (i, _) in pages.iter().enumerate() {
                    let page_number = (i + 1) as u32;
                    let text = document.extract_text(&[page_number]);
                    texts.push(text.unwrap_or_default());
                }

                println!("Text on page {}: {}", 42, texts[41]);
                Ok(texts)
            }
            Err(err) => {
                eprintln!("Error: {}", err);
                //return Err(io::Error::new(io::ErrorKind::InvalidData, err.into()));
                return Err(*Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    "your message here",
                )));
                //Err(err.into());
                //                return Err(*Box::from(io::Error::new(err.to_string()))); //Err(err.into());
            }
        }
    }
    Ok(())
}
