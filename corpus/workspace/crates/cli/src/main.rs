//! The binary. This is what ships, and the target a sweep is pointed at.

fn main() {
    let text = "# the profile\nopt-level = 3\nlto = fat\ncodegen-units = 1\n";
    println!("{}", engine::describe(text));
    println!("{} setting(s)", parser::parse(text).len());
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_binary_sees_both_libraries() {
        let text = "a = 1\n";
        assert_eq!(engine::describe(text), "a=1");
        assert_eq!(parser::parse(text).len(), 1);
    }
}
