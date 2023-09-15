#![allow(unused)]

#[macro_use]
extern crate tantivy;
use tantivy::{collector::TopDocs, query::QueryParser, schema::*, Index, ReloadPolicy};
use tempfile::TempDir;

fn main() -> tantivy::Result<()> {
    // Temporary directory for index
    let index_path = TempDir::new()?;

    // Defining the schema
    let mut schema_builder = Schema::builder();

    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT);

    let schema = schema_builder.build();

    // Indexing documents
    let index = Index::create_in_dir(&index_path, schema.clone())?;

    // we give tantivy a budget of 50MB.
    // Using a bigger heap for the indexer may increase throughput,
    // but 50 MB is already plenty.
    let mut index_writer = index.writer(50_000_000)?;

    // Adding documents
    // Get fields from schema
    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();

    let mut old_man_doc = Document::default();
    old_man_doc.add_text(title, "The Old Man and the Sea");
    old_man_doc.add_text(
        body,
        "He was an old man who fished alone in a skiff in the Gulf Stream and \
         he had gone eighty-four days now without taking a fish.",
    );

    // Add document to the `IndexWriter`
    index_writer.add_document(old_man_doc);

    // For convenience, tantivy also comes with a macro
    // to reduce the boilerplate above.
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
    ));

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
    ));

    // We need to call `.commit()`` explicitly to force
    // the `index_writer` to finish processing the documents
    // in the queue, flush the current index to the disk,
    // and advertise the existence of new documents.
    // This call is blocking.
    index_writer.commit()?;

    // If `.commit()` returns correctly, then all of the documents
    // that have been added are guaranteed to be persistently indexed.
    // In the scenario of a crash or a power failure,
    // tantivy behaves as if has rolled back to its last commit.

    // SEARCHING
    // A reader is required to get search the index.
    // It acts as a `Searcher` pool that reloads itself,
    // depending on a `ReloadPolicy`.
    //
    // For a search server we shall typically create one reader
    // for the entire lifetime of your program,
    // and acquire a new searcher for every single request.
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommit)
        .try_into()?;

    // We now need to acquire a searcher.
    // A searcher points to snapshotted, immutable version of the index.
    // Some search experience might require more than one query.
    // Using the same searcher ensures that all of these queries will run on the same version of the index.
    //
    // Acquiring a `searcher` is very cheap.
    //
    // We should acquire a searcher every time we start processing
    // a request and and release it right after our query is finished.
    let searcher = reader.searcher();

    // QUERY
    //
    // The query parser can interpret human queries.
    //
    // Here, if the user does not specify which field they want to search,
    // tantivy will search in both title and body.
    let query_parser = QueryParser::for_index(&index, vec![title, body]);

    // QueryParser may fail if the query is not in the right format.
    // For user facing applications, this can be a problem.

    // A query defines a set of documents, as well as the way they should be scored.
    // A query created by the query parser is scored according to a metric called Tf-Idf,
    // and will consider any document matching at least one of our terms.
    let query = query_parser.parse_query("sea whale")?;

    // COLLECTORS
    //
    // Keeping track of our top 10 best documents is the role of the TopDocs.
    //
    // We can now perform our query.
    let top_docs = searcher.search(&query, &TopDocs::with_limit(10))?;

    // The actual documents still need to be retrieved from Tantivy’s store.
    // Since the body field was not configured as stored, the document returned will only contain a title.
    for (_score, doc_address) in top_docs {
        let retrieved_doc = searcher.doc(doc_address)?;
        println!("{}", schema.to_json(&retrieved_doc));
    }

    Ok(())
}
