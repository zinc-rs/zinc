fn main() {
println!("{:?}", r#"Parsing JSON..."#);let raw = r#"{"name": "Zinc", "versions": [1, 2]}"#;let obj = zinc_std::json::parse(raw);println!("{:?}", r#"Object parsed."#);let name = zinc_std::json::get(&obj, r#"name"#);println!("{:?}", r#"Name:"#);println!("{:?}", name);let versions = zinc_std::json::get(&obj, r#"versions"#);let v1 = zinc_std::json::at(&versions, 0);println!("{:?}", r#"First Version:"#);println!("{:?}", v1);
 zinc_std::check_leaks();
}