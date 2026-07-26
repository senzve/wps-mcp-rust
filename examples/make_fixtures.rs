fn main() {
    let mut book = umya_spreadsheet::new_file();
    book.get_sheet_mut(&0).unwrap().set_name("Sheet1");
    {
        let sheet = book.get_sheet_by_name_mut("Sheet1").unwrap();
        sheet.get_cell_mut("A1").set_value("Name");
        sheet.get_cell_mut("B1").set_value("Age");
        sheet.get_cell_mut("A2").set_value("Alice");
        sheet.get_cell_mut("B2").set_value_number(30.0);
        sheet.get_cell_mut("A3").set_value("Bob");
        sheet.get_cell_mut("B3").set_value_number(25.0);
    }
    book.new_sheet("Sheet2").unwrap();
    book.get_sheet_by_name_mut("Sheet2")
        .unwrap()
        .get_cell_mut("A1")
        .set_value("X");
    std::fs::create_dir_all("tests/fixtures").unwrap();
    umya_spreadsheet::writer::xlsx::write(&book, "tests/fixtures/sample.xlsx").unwrap();
    println!("wrote sample.xlsx");
}
