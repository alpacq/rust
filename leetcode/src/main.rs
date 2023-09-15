fn find_itinerary(tickets: Vec<Vec<String>>) -> Vec<String> {
  let mut tickets_clone = tickets.clone();
  let mut result: Vec<String> = Vec::new();
  let mut possible_starts: Vec<&Vec<String>> = tickets.iter().filter(|flight| flight[0] == "JFK").collect();
  possible_starts.sort();
  if let Some(start) = possible_starts.first() {
    result.push(start[0].to_string());
    result.push(start[1].to_string());
    tickets_clone.remove(tickets.iter().position(|flight| flight[0] == start[0] && flight[1] == start[1]).unwrap());
    while tickets_clone.len() > 0 {
      let mut possible_next: Vec<&Vec<String>> = tickets_clone.iter().filter(|flight| &flight[0] == result.last().unwrap()).collect();
      possible_next.sort();
      if let Some(next) = possible_next.first() {
        result.push(next[1].to_string());
        tickets_clone.remove(tickets_clone.iter().position(|flight| flight[0] == next[0] && flight[1] == next[1]).unwrap());
      } else {
        return result;
      }
    }
  } else {
    return result;
  }

  println!("{:?}", result);
  result
}

fn main() {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn find_itinerary_case1() {
    let tickets = vec![vec!["JFK".to_string(), "SFO".to_string()], vec!["JFK".to_string(), "ATL".to_string()], vec!["SFO".to_string(), "ATL".to_string()], vec!["ATL".to_string(), "JFK".to_string()], vec!["ATL".to_string(), "SFO".to_string()]];
    assert_eq!(vec!["JFK".to_string(), "ATL".to_string(), "JFK".to_string(), "SFO".to_string(), "ATL".to_string(), "SFO".to_string()], find_itinerary(tickets));
  }

  #[test]
  fn find_itinerary_case2() {
    let tickets = vec![vec!["MUC".to_string(), "LHR".to_string()], vec!["JFK".to_string(), "MUC".to_string()], vec!["SFO".to_string(), "SJC".to_string()], vec!["LHR".to_string(), "SFO".to_string()]];
    assert_eq!(vec!["JFK".to_string(), "MUC".to_string(), "LHR".to_string(), "SFO".to_string(), "SJC".to_string()], find_itinerary(tickets));
  }

  #[test]
  fn find_itinerary_case3() {
    let tickets = vec![vec!["JFK".to_string(), "KUL".to_string()], vec!["JFK".to_string(), "NRT".to_string()], vec!["NRT".to_string(), "JFK".to_string()]];
    assert_eq!(vec!["JFK".to_string(),"NRT".to_string(),"JFK".to_string(),"KUL".to_string()], find_itinerary(tickets));
  }
}
