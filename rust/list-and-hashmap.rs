extern crate hadean_std;

use hadean_std::List;
use hadean_std::stats::jaccard;

fn main() {
	// Get a list of user ids and interests
	let data_list = List::load_s3("s3://bucket/log.file");
	let data_rows = data_list.split(b'\n');
	let rows = data_rows.map(|row| row.split(b' '));              // [[userId, interest],...]
	// Get all the interests for each user
	let users = rows.group_by(|row| (row[0], row[1]));            // {userId:[interest,...],...}
	// Get all combinations of users with their interests
	let edges = users.cartesian().filter(|(k, _v)| k[0] != k[1]); // {(userId1,userId2):([interest1,...],[interest2,...]),...}
	// Measure the similarity of user interests
	let scores = edges.map(|(k, v)| jaccard(v[0], v[1]));          // {(userId1,userId2):0.345,...}
	// Make a list of user pairs with similarity above some threshold
	let matches = scores.filter(|(k, v)| v >= 0.8).keys();        // [(userId1,userId2),...]
	// Store the matches in s3
	let matches_data = matches.map(|(u1, u2)| format!("{} {}", u1, u2)).join(b'\n');
	matches_data.store_s3("s3://bucket/matches.file");
}



