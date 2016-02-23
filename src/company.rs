use postgres::Connection;
use postgres_array::Array;

#[derive(Debug)]
pub struct Company {
  pub presented_talents: Vec<i32>,
}

impl Company {
  pub fn find(conn: &Connection, id: &i32) -> Option<Company> {
    let response = &conn.query(
      "SELECT * FROM companies
        WHERE id = $1
        LIMIT 1", &[&id]
    ).unwrap();

    let mut results = response.iter().map(|row| {
      let presented_talents: Array<String> = row.get("presented_talents");

      Company {
        presented_talents: presented_talents.iter()
                                            .map(|s| s.parse::<i32>().unwrap())
                                            .collect::<Vec<i32>>(),
      }
    }).collect::<Vec<Company>>();

    results.pop()
  }
}
