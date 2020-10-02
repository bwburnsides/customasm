use crate::*;

//mod excerpt;
//mod expr;
//mod cpudef;
//mod asm;


// generated by build script
include!(concat!(env!("OUT_DIR"), "/test.rs"));


pub struct TestFile
{
    name: String,
    contents: String,
    output: util::BitVec,
    messages: Vec<TestMessageExpectation>,
}


pub struct TestMessageExpectation
{
    filename: String,
    kind: diagn::MessageKind,
    line: usize,
    excerpt: String,
}


impl TestFile
{
    fn new() -> TestFile
    {
        TestFile
        {
            name: "".to_string(),
            contents: "".to_string(),
            output: util::BitVec::new(),
            messages: Vec::new(),
        }
    }
}


pub fn parse_subfiles<T: Into<String>>(contents: T, up_to_subfile: &str) -> Result<Vec<TestFile>, ()>
{
    let mut files = Vec::new();
    let mut cur_subfile = TestFile::new();

    let mut auto_subfile_index = 1;
    let mut line_num = 0;

    for line in contents.into().lines()
    {
        if line.starts_with("; :::")
        {
            if cur_subfile.name.len() > 0
            {
                let cur_name = cur_subfile.name.clone();

				files.retain(|f: &TestFile| f.name != cur_subfile.name);
                files.push(cur_subfile);

				if cur_name == up_to_subfile
				{
                    cur_subfile = TestFile::new();
					break;
                }
			}
			
            let mut name = format!("{}", &line.get(5..).unwrap().trim());

            if name.len() == 0
            {
                name = format!("{}", auto_subfile_index);
                auto_subfile_index += 1;
			}
			
            cur_subfile = TestFile::new();
            cur_subfile.name = name;
            line_num = 0;
        }
        else
        {
            cur_subfile.contents.push_str(&line);
            cur_subfile.contents.push_str("\n");

            if let Some(value_index) = line.find("; = ")
            {
                let value_str = line.get((value_index + 4)..).unwrap().trim();
                if value_str != "0x"
                {
                    let value = syntax::excerpt_as_bigint(None, value_str, &diagn::Span::new_dummy()).unwrap();
                    
                    let index = cur_subfile.output.len();
                    cur_subfile.output.write_bigint(index, value);
                }
            }
            else if line.find("; error: ").is_some()
            {
                let messages = line.get(2..).unwrap().split("/").map(|s| s.trim());
                for message in messages
                {
                    if let Some(excerpt_index) = message.find("error: ")
                    {
                        let mut filename = cur_subfile.name.clone();
                        let mut line = line_num;
                        let mut excerpt = message.get((excerpt_index + 7)..).unwrap().trim().to_string();

                        if let Some(colon_index) = excerpt.find(":")
                        {
                            filename = excerpt.get(0..colon_index).unwrap().trim().to_string();

                            excerpt = excerpt.get((colon_index + 1)..).unwrap().trim().to_string();

                            let next_colon_index = excerpt.find(":").unwrap();
                            line = excerpt.get(0..next_colon_index).unwrap().parse::<usize>().unwrap() - 1;
                        
                            excerpt = excerpt.get((next_colon_index + 1)..).unwrap().trim().to_string();
                        }
            
                        cur_subfile.messages.push(TestMessageExpectation
                        {
                            kind: diagn::MessageKind::Error,
                            filename,
                            line,
                            excerpt,
                        });
                    }
                }
            }

            line_num += 1;
        }
    }

    if cur_subfile.name.len() > 0
    {
        files.push(cur_subfile);
    }

    Ok(files)
}


pub fn test_subfile(filepath: &str, subfilename: &str)
{
	let contents = std::fs::read_to_string(&filepath).unwrap();
	
	let subfiles = parse_subfiles(contents, subfilename).unwrap();
	let mut fileserver = util::FileServerMock::new();

	for file in &subfiles
	{
		fileserver.add(file.name.clone(), file.contents.clone());
	}

	let subfile = subfiles.iter().find(|f| f.name == subfilename).unwrap();

	let report = diagn::RcReport::new();
	let mut assembler = asm::Assembler::new();

    use util::FileServer;
	if fileserver.exists("include")
	{
		assembler.register_file("include");
	}

	assembler.register_file(subfilename);
	let output = assembler.assemble(report.clone(), &mut fileserver, 10).ok();
	
	let mut msgs = Vec::<u8>::new();
	report.print_all(&mut msgs, &fileserver);
    print!("{}", String::from_utf8(msgs).unwrap());
    
    let mut has_msg_mismatch = false;
    for msg in &subfile.messages
    {
        if !report.has_message_at(&fileserver, &msg.filename, msg.kind, msg.line, &msg.excerpt)
        {
            println!("\n\
                > test failed -- diagnostics mismatch\n\
                > expected: `{}` at file `{}`, line {}\n",
                msg.excerpt, msg.filename, msg.line);

            has_msg_mismatch = true;
        }
    }
    
    if has_msg_mismatch
    {
        panic!("test failed");
    }

    if subfile.messages.len() != report.len()
    {
        println!("\n\
            > test failed -- diagnostics mismatch\n\
            > expected {} messages, got {}\n",
            subfile.messages.len(), report.len());
            
        panic!("test failed");
    }
    
    let output = output.unwrap_or(util::BitVec::new());

    if format!("{:x}", output) != format!("{:x}", subfile.output)
    {
        println!("\n\
            > test failed -- output mismatch\n\
            > got:      0x{:x}\n\
            > expected: 0x{:x}\n",
            &output, &subfile.output);
            
        panic!("test failed");
    }
}