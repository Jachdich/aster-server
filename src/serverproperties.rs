use std::io::Read;

pub struct ServerProperties {
    pub name: String,
    pub pfp: String,
}

impl ServerProperties {
    pub fn load() -> Self {
        let mut server_name = String::new();
        let server_icon: String;
        
        let icon_file = std::fs::File::open("icon.png");
        match icon_file {
            Ok(mut file) => {
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer).unwrap();
                server_icon = base64::encode(buffer);
            }
            Err(_) => {
                panic!("Please provide a file icon.png for the server icon");
            }
        }

        let name_file = std::fs::File::open("name.txt");
        match name_file {
            Ok(mut file) => {
                file.read_to_string(&mut server_name).unwrap();
                server_name.pop();
            }
            Err(_) => {
                panic!("Please provide a file name.txt with the server name");
            }
        }

        ServerProperties {
            name: server_name,
            pfp: server_icon
        }
    }
}