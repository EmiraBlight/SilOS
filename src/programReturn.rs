use alloc::string::String;

pub struct ProcessError{
    pub error_code: String,
}


impl ProcessError{
    pub fn error_str (&self)->&String{
        &self.error_code
    }
}

pub struct Success{
    pub success_code: String,
    pub print_code:bool,
}

impl Success{
    pub fn is_print(&self)-> &bool{
        return &self.print_code
    }

    pub fn success_str(&self) -> &String{
        return &self.success_code
    }

}

