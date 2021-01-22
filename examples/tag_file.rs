//! Read and write vorbis comment metadata.

use oggvorbismeta::{read_comment_header, replace_comment_header, CommentHeader, VorbisComments};
use std::env;
use std::fs::File;
use std::io::Cursor;

fn main() -> anyhow::Result<()> {
    let file_in = env::args().nth(1).expect("Please specify an input file.");
    let file_out = env::args().nth(2).expect("Please specify an output file.");
    println!("Opening files: {}, {}", file_in, file_out);

    // Open files
    let mut f_in_disk = File::open(file_in).expect("Can't open file");
    let mut f_in_ram: Vec<u8> = vec![];

    println!("Copy input file to buffer");
    std::io::copy(&mut f_in_disk, &mut f_in_ram)?;

    let f_in = Cursor::new(&f_in_ram);
    println!("Read comments from file");
    let read_comments = read_comment_header(f_in)?;

    let tag_names = read_comments.get_tag_names();
    println!("Existing tags: {:?}", tag_names);
    for tag in tag_names.iter() {
        println!(
            "Existing tag: {}, {:?}",
            tag,
            read_comments.get_tag_multi(tag)
        );
    }

    let f_in = Cursor::new(&f_in_ram);
    let mut new_comment = CommentHeader::new();
    println!("Make new comment header");
    new_comment.set_vendor("Ogg");
    new_comment.add_tag_single("artist", "Some Guy");
    new_comment.add_tag_single("artist", "Another Dude");
    new_comment.add_tag_single("album", "Greatest Hits");
    new_comment.add_tag_single("tracknumber", "3");
    new_comment.add_tag_single("title", "A very good song");
    new_comment.add_tag_single("date", "1997");

    let tag_names = new_comment.get_tag_names();
    println!("New tags: {:?}", tag_names);
    for tag in tag_names.iter() {
        println!("New tag: {}, {:?}", tag, new_comment.get_tag_multi(tag));
    }

    println!("Insert new comments");
    let f_out = File::create(file_out)?;
    replace_comment_header(f_in, f_out, new_comment)?;

    Ok(())
}
