use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Default, Debug)]
struct FileInfo {
    filetype: String,
    steps: usize,
    blanks: usize,
    comments: usize,
    files: usize,
    bytes: usize,
}

#[derive(Default, Debug)]
struct CntResult {
    info: Vec<FileInfo>,
    input_path: String,
    all_steps: usize,
    all_blanks: usize,
    all_comments: usize,
    all_files: usize,
    all_bytes: usize,
}

const MAX_CAPACITY: usize = 1024 * 1024;
const CONCURRENCY_THRESHOLD: usize = 6;

fn count(files: Vec<String>, input_path: String) -> io::Result<CntResult> {
    let mut result = CntResult {
        input_path: input_path.clone(),
        ..Default::default()
    };
    let buf_map: Arc<Mutex<HashMap<String, FileInfo>>> = Arc::new(Mutex::new(HashMap::new()));
    let len_files = files.len();

    if len_files >= CONCURRENCY_THRESHOLD {
        let chunk_size = (len_files + 2) / 3;
        let chunks: Vec<Vec<String>> = files.chunks(chunk_size).map(|chunk| chunk.to_vec()).collect();

        let mut handles = vec![];
        for chunk in chunks {
            let buf_map = Arc::clone(&buf_map);
            let handle = thread::spawn(move || process_files(chunk, buf_map));
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    } else {
        for file in files {
            process_file(file, &buf_map)?;
        }
    }

    let buf_map = buf_map.lock().unwrap();
    for (_, file_info) in buf_map.iter() {
        result.info.push(file_info.clone());
    }
    result.assign_alls();
    Ok(result)
}

fn count_file(file: &str) -> io::Result<FileInfo> {
    let mut info = FileInfo::default();
    let path = Path::new(file);
    let file = File::open(path)?;
    let mut scanner = io::BufReader::new(file);

    info.filetype = ret_file_type(path);

    let mut in_block_comment = false;
    for line in scanner.lines() {
        let line = line?.trim().to_string();
        info.steps += 1;
        info.bytes += line.len() + 1; // +1 for newline character

        if line.is_empty() {
            info.blanks += 1;
            continue;
        }

        if is_single_comment(&line) {
            info.comments += 1;
            continue;
        }

        if is_begin_block_comments(&line) {
            in_block_comment = true;
            info.comments += 1;
            continue;
        }

        if in_block_comment {
            info.comments += 1;
            if is_end_block_comments(&line) {
                in_block_comment = false;
            }
        }
    }
    Ok(info)
}

fn process_files(files: Vec<String>, buf_map: Arc<Mutex<HashMap<String, FileInfo>>>) {
    for file in files {
        if let Err(err) = process_file(file, &buf_map) {
            eprintln!("Failed to count lines in file {}: {}", file, err);
        }
    }
}

fn process_file(file: String, buf_map: &Arc<Mutex<HashMap<String, FileInfo>>>) -> io::Result<()> {
    let file_info = count_file(&file)?;
    let mut buf_map = buf_map.lock().unwrap();

    let entry = buf_map.entry(file_info.filetype.clone()).or_insert_with(FileInfo::default);
    entry.steps += file_info.steps;
    entry.blanks += file_info.blanks;
    entry.comments += file_info.comments;
    entry.bytes += file_info.bytes;
    entry.files += 1;

    Ok(())
}

fn ret_file_type(path: &Path) -> String {
    match path.extension() {
        Some(ext) => ext.to_string_lossy().to_string(),
        None => path.file_name().unwrap().to_string_lossy().to_string(),
    }
}

impl CntResult {
    fn assign_alls(&mut self) {
        for info in &self.info {
            self.all_steps += info.steps;
            self.all_blanks += info.blanks;
            self.all_comments += info.comments;
            self.all_files += info.files;
            self.all_bytes += info.bytes as i64;
        }
    }
}

lazy_static::lazy_static! {
    static ref SINGLE_COMMENT_PREFIXES: HashMap<&'static str, ()> = {
        let mut m = HashMap::new();
        m.insert("//", ());
        m.insert("///", ());
        m.insert("#", ());
        m.insert("!", ());
        m.insert("--", ());
        m.insert("%", ());
        m.insert(";", ());
        m.insert("#;", ());
        m.insert("⍝", ());
        m.insert("rem ", ());
        m.insert("::", ());
        m.insert(":  ", ());
        m.insert("'", ());
        m
    };

    static ref BLOCK_COMMENT_PREFIXES: HashMap<&'static str, ()> = {
        let mut m = HashMap::new();
        m.insert("/*", ());
        m.insert("/**", ());
        m.insert("--", ());
        m.insert("<!--", ());
        m.insert("<%--", ());
        m.insert("////", ());
        m.insert("/+", ());
        m.insert("/++", ());
        m.insert("(*", ());
        m.insert("{-", ());
        m.insert("\"\"\"", ());
        m.insert("'''", ());
        m.insert("#=", ());
        m.insert("--[[", ());
        m.insert("%{", ());
        m.insert("#[", ());
        m.insert("=pod", ());
        m.insert("=comment", ());
        m.insert("=begin", ());
        m.insert("<#", ());
        m.insert("#|", ());
        m
    };

    static ref BLOCK_COMMENT_SUFFIXES: HashMap<&'static str, ()> = {
        let mut m = HashMap::new();
        m.insert("*/", ());
        m.insert("**/", ());
        m.insert("-->", ());
        m.insert("--%>", ());
        m.insert("--", ());
        m.insert("+/", ());
        m.insert("*)", ());
        m.insert("-}", ());
        m.insert("%}", ());
        m.insert("=#", ());
        m.insert("=cut", ());
        m.insert("=end", ());
        m.insert("--]]", ());
        m.insert("]#", ());
        m.insert("#>", ());
        m.insert("\"\"\"", ());
        m.insert("'''", ());
        m.insert("|#", ());
        m
    };
}

fn is_single_comment(line: &str) -> bool {
    SINGLE_COMMENT_PREFIXES.keys().any(|&prefix| line.starts_with(prefix))
}

fn is_begin_block_comments(line: &str) -> bool {
    BLOCK_COMMENT_PREFIXES.keys().any(|&prefix| line.starts_with(prefix))
}

fn is_end_block_comments(line: &str) -> bool {
    BLOCK_COMMENT_SUFFIXES.keys().any(|&suffix| line.ends_with(suffix))
}
