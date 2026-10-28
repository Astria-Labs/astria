use crate::config::user::User;

pub struct ConfigCommand;

impl ConfigCommand {

    pub fn execute(global: bool, args: Vec<String>) -> std::io::Result<()> {
    
        if args.len() >= 2 {
            let key = &args[0];
            let raw = args[1..].join(" ");
            let value = raw.trim_matches('"').trim_matches('\'');

            match User::set(global, key, value) {
                Ok(_) => println!("Your global configurations have been set."),
                Err(e) => eprintln!("Failed to set: {}", e),
            }
            
        } else {
            println!("Usage: ast config [--global] user.name \"Your nane\"");
            println!("       ast config [--global] user.email \"your@email.com\"");
        }
    
        Ok(())
    }
}