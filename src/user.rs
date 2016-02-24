use postgres::Connection;
use postgres_array::Array;

#[derive(Debug, RustcEncodable)]
pub struct User {
  pub id:              i32,
  pub first_name:      Option<String>,
  pub last_name:       Option<String>,
  pub headline:        Option<String>,
  pub work_roles:      Vec<String>,
  pub work_languages:  Vec<String>,
  pub work_experience: Option<String>,
  pub avatar_url:      Option<String>
}

impl User {
  pub fn find(conn: &Connection, id: &i32) -> Option<User> {
    conn.query("SELECT * FROM users
                INNER JOIN talents ON users.id = talents.id
                WHERE users.id = $1
                LIMIT 1", &[&id])
        .unwrap()
        .iter()
        .map(|row| {
          let work_roles:     Array<String> = row.get("work_roles");
          let work_languages: Array<String> = row.get("work_languages");

          User {
            id:              row.get("id"),
            first_name:      row.get("firstname"),
            last_name:       row.get("lastname"),
            headline:        row.get("headline"),
            work_roles:      work_roles.iter()
                                       .cloned()
                                       .collect::<Vec<String>>(),
            work_languages:  work_languages.iter()
                                           .cloned()
                                           .collect::<Vec<String>>(),
            work_experience: row.get("work_experience"),
            avatar_url:      row.get("avatar_url"),
          }
        })
        .collect::<Vec<User>>()
        .pop()
  }
}
