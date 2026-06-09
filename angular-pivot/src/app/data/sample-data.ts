// The interface you specified
export interface DataRow {
  fields: Record<string, string>;
}

// The shape of your JSON data (only the relevant part)
interface ChampionsLeagueData {
  competition: string;
  most_successful_club: string;
  winners: Array<{
    season: string;
    winner: string;
    winner_country: string;
    runner_up: string;
    runner_up_country: string;
    score: string;
    venue: string;
  }>;
}

/**
 * Converts the raw JSON object into an array of DataRow.
 * Each DataRow corresponds to one Champions League winner entry.
 *
 * @param jsonData - The parsed JSON object matching the structure above
 * @returns An array of DataRow, each containing a flat record of string key-value pairs
 */
export function populateDataRows(jsonData: ChampionsLeagueData): DataRow[] {
  const rows: DataRow[] = [];

  for (const winner of jsonData.winners) {
    const fields: Record<string, string> = {
      season: winner.season,
      winner: winner.winner,
      winner_country: winner.winner_country,
      runner_up: winner.runner_up,
      runner_up_country: winner.runner_up_country,
      score: winner.score,
      venue: winner.venue,
      // Optional: include competition metadata if desired
      competition: jsonData.competition,
      most_successful_club: jsonData.most_successful_club,
    };
    rows.push({ fields });
  }

  return rows;
}
