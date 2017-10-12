error_chain! {
    foreign_links {
        Io(::std::io::Error);
        PIE(::std::num::ParseIntError);
        PFE(::std::num::ParseFloatError);
        Regex(::regex::Error);
    }
}
