fn main() {
let url = "sqlite:test.db";let url_create = "sqlite:test.db?mode=rwc";println!("{:?}", "Creating table...");zinc_std::db::query(url_create, "CREATE TABLE IF NOT EXISTS logs (msg TEXT);");println!("{:?}", "Inserting log...");zinc_std::db::query(url_create, "INSERT INTO logs VALUES ('Hello from Zinc');");println!("{:?}", "Reading logs...");let data = zinc_std::db::query(url_create, "SELECT * FROM logs;");println!("{:?}", data);
 zinc_std::check_leaks();
}