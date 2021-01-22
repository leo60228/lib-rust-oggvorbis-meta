//! Read and write vorbiscomment metadata.

#![warn(missing_docs)]

use ogg::writing::PacketWriteEndInfo;
use ogg::{Packet, PacketReader, PacketWriter};
use std::convert::TryInto;
use std::io::{Cursor, Read, Seek};

/// A comment header.
pub type CommentHeader = lewton::header::CommentHeader;

/// A holder of Vorbis comments.
pub trait VorbisComments {
    /// Construct a VorbisComments from its contents.
    fn from(vendor: String, comment_list: Vec<(String, String)>) -> Self;

    /// Create an empty VorbisContents.
    fn new() -> Self;

    /// Get all tag names used.
    fn get_tag_names(&self) -> Vec<String>;

    /// Get one tag.
    fn get_tag_single(&self, tag: &str) -> Option<&str>;

    /// Get one instance of a tag.
    fn get_tag_multi(&self, tag: &str) -> Vec<&str>;

    /// Remove a tag.
    fn clear_tag(&mut self, tag: &str);

    /// Add a tag.
    fn add_tag_single(&mut self, tag: &str, value: &str);

    /// Add multiple instances of a tag.
    fn add_tag_multi(&mut self, tag: &str, values: &[&str]);

    /// Get the vendor.
    fn get_vendor(&self) -> &str;

    /// Set the vendor.
    fn set_vendor(&mut self, vend: &str);
}

impl VorbisComments for CommentHeader {
    fn from(vendor: String, comment_list: Vec<(String, String)>) -> CommentHeader {
        CommentHeader {
            vendor,
            comment_list,
        }
    }

    fn new() -> CommentHeader {
        CommentHeader {
            vendor: "".to_string(),
            comment_list: Vec::new(),
        }
    }

    fn get_tag_names(&self) -> Vec<String> {
        let mut names = self
            .comment_list
            .iter()
            .map(|comment| comment.0.to_lowercase())
            .collect::<Vec<String>>();
        names.sort_unstable();
        names.dedup();
        names
    }

    fn get_tag_single(&self, tag: &str) -> Option<&str> {
        let tags = self.get_tag_multi(tag);
        if !tags.is_empty() {
            Some(tags[0])
        } else {
            None
        }
    }

    fn get_tag_multi(&self, tag: &str) -> Vec<&str> {
        self.comment_list
            .iter()
            .filter(|comment| comment.0.to_lowercase() == tag.to_lowercase())
            .map(|comment| &*comment.1)
            .collect::<Vec<&str>>()
    }

    fn clear_tag(&mut self, tag: &str) {
        self.comment_list
            .retain(|comment| comment.0.to_lowercase() != tag.to_lowercase());
    }

    fn add_tag_single(&mut self, tag: &str, value: &str) {
        self.comment_list
            .push((tag.to_lowercase(), value.to_string()));
    }

    fn add_tag_multi(&mut self, tag: &str, values: &[&str]) {
        for value in values.iter() {
            self.comment_list
                .push((tag.to_lowercase(), value.to_string()));
        }
    }

    fn get_vendor(&self) -> &str {
        &self.vendor
    }

    fn set_vendor(&mut self, vend: &str) {
        self.vendor = vend.to_string();
    }
}

/// Write out a comment header.
pub fn make_comment_header(header: &CommentHeader) -> Vec<u8> {
    // Signature
    let start = [3u8, 118, 111, 114, 98, 105, 115];

    // Vendor number of bytes as u32
    let vendor = header.vendor.as_bytes();
    let vendor_len: u32 = vendor.len().try_into().unwrap();

    // End byte
    let end: u8 = 1;

    let mut new_packet: Vec<u8> = vec![];

    // Write start
    new_packet.extend(start.iter().cloned());

    // Write vendor
    new_packet.extend(vendor_len.to_le_bytes().iter().cloned());
    new_packet.extend(vendor.iter().cloned());

    // Write number of comments
    let comment_nbr: u32 = header.comment_list.len().try_into().unwrap();
    new_packet.extend(comment_nbr.to_le_bytes().iter().cloned());

    let mut commentstrings: Vec<String> = vec![];
    // Write each comment
    for comment in header.comment_list.iter() {
        commentstrings.push(format!("{}={}", comment.0, comment.1));
        let comment_len: u32 = commentstrings
            .last()
            .unwrap()
            .as_bytes()
            .len()
            .try_into()
            .unwrap();
        new_packet.extend(comment_len.to_le_bytes().iter().cloned());
        new_packet.extend(commentstrings.last().unwrap().as_bytes().iter().cloned());
    }
    new_packet.push(end);

    new_packet
}

/// Read a comment header.
pub fn read_comment_header<T: Read + Seek>(f_in: T) -> CommentHeader {
    let mut reader = PacketReader::new(f_in);

    let packet: Packet = reader.read_packet_expected().unwrap();
    let stream_serial = packet.stream_serial();

    let mut packet: Packet = reader.read_packet_expected().unwrap();

    while packet.stream_serial() != stream_serial {
        packet = reader.read_packet_expected().unwrap();
    }

    lewton::header::read_header_comment(&packet.data).unwrap()
}

/// Replace the comment header of a file.
pub fn replace_comment_header<T: Read + Seek>(
    f_in: T,
    new_header: CommentHeader,
) -> Cursor<Vec<u8>> {
    let new_comment_data = make_comment_header(&new_header);

    let f_out_ram: Vec<u8> = vec![];
    let mut f_out = Cursor::new(f_out_ram);

    let mut reader = PacketReader::new(f_in);
    let mut writer = PacketWriter::new(&mut f_out);

    let mut header_done = false;
    loop {
        let rp = reader.read_packet();
        match rp {
            Ok(r) => {
                match r {
                    Some(mut packet) => {
                        let inf = if packet.last_in_stream() {
                            PacketWriteEndInfo::EndStream
                        } else if packet.last_in_page() {
                            PacketWriteEndInfo::EndPage
                        } else {
                            PacketWriteEndInfo::NormalPacket
                        };
                        if !header_done {
                            let comment_hdr = lewton::header::read_header_comment(&packet.data);
                            match comment_hdr {
                                Ok(_hdr) => {
                                    // This is the packet to replace
                                    packet.data = new_comment_data.clone();
                                    header_done = true;
                                }
                                Err(_error) => {}
                            }
                        }
                        let lastpacket = packet.last_in_stream() && packet.last_in_page();
                        let stream_serial = packet.stream_serial();
                        let absgp_page = packet.absgp_page();
                        writer
                            .write_packet(
                                packet.data.into_boxed_slice(),
                                stream_serial,
                                inf,
                                absgp_page,
                            )
                            .unwrap();
                        if lastpacket {
                            break;
                        }
                    }
                    // End of stream
                    None => break,
                }
            }
            Err(error) => {
                println!("Error reading packet: {:?}", error);
                break;
            }
        }
    }
    f_out.seek(std::io::SeekFrom::Start(0)).unwrap();
    f_out
}
