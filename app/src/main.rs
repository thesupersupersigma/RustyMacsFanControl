mod lang;

fn main() {
    let lang = lang::Language::english();
    println!("{}", lang.get("AboutDialog/title"));
}
