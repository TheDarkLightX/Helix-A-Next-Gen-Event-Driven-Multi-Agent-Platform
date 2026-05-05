import { logMessage as log } from "../host/log";
import { httpGet, HttpResponse } from "../host/http";
import { HostFunctionError, BufferTooSmallError } from "../utils/errors";

// Configuration
const DEFAULT_CITY = "NewYork"; // Default city for weather
const WEATHER_API_TIMEOUT_MS: u32 = 10000; // 10 seconds

export function agent_main(): void {
  log("Daily Briefing Agent started.");

  // --- Fetch Weather ---
  const city = DEFAULT_CITY; // Later, this could come from agent state
  // Using a format that's easier to parse without full JSON support
  // %l: Location, %t: Temperature, %C: Condition text
  const weatherUrl = "http://wttr.in/" + city + "?format=%l:%t %C";
  log("Fetching weather for " + city + " from " + weatherUrl);

  let weatherResponse: HttpResponse | null = httpGet(weatherUrl, null, WEATHER_API_TIMEOUT_MS);

  if (weatherResponse) {
    log("Weather API response status: " + weatherResponse.status.toString());
    if (weatherResponse.status == 200) {
      const weatherData = weatherResponse.text();
      if (weatherData !== null) {
        log("Raw weather data: " + weatherData);
        parseAndLogWeather(weatherData);
      } else {
        log("Weather API returned empty body.");
      }
    } else {
      log("Weather API request failed with status: " + weatherResponse.status.toString());
      const errorBody = weatherResponse.text();
      if (errorBody !== null) {
        log("Weather API error body: " + errorBody);
      }
    }
  } else {
    log("Weather API request failed (httpGet returned null). URL: " + weatherUrl);
  }

  // --- Fetch Quote of the Day ---
  const quoteUrl = "https://api.quotable.io/random";
  log("Fetching quote of the day from " + quoteUrl);
  let quoteResponse: HttpResponse | null = httpGet(quoteUrl, null, WEATHER_API_TIMEOUT_MS); // Reusing timeout

  if (quoteResponse) {
    log("Quote API response status: " + quoteResponse.status.toString());
    if (quoteResponse.status == 200) {
      const quoteJson = quoteResponse.text();
      if (quoteJson !== null) {
        log("Raw quote data: " + quoteJson);
        parseAndLogQuote(quoteJson);
      } else {
        log("Quote API returned empty body.");
      }
    } else {
      log("Quote API request failed with status: " + quoteResponse.status.toString());
      const errorBody = quoteResponse.text();
      if (errorBody !== null) {
        log("Quote API error body: " + errorBody);
      }
    }
  } else {
    log("Quote API request failed (httpGet returned null). URL: " + quoteUrl);
  }

  log("Daily Briefing Agent finished.");
}

function parseAndLogWeather(weatherData: String): void {
  // Example format: "New York: +72°F Clear"
  // We'll split by ":" first, then by " " for temp and condition.
  // This is a very basic parser and might break if wttr.in changes format.
  const parts = weatherData.split(":", 2);
  if (parts.length >= 2) {
    const location = parts[0].trim();
    const rest = parts[1].trim();

    const tempAndCondition = rest.split(" ", 2); // Limit to 2 parts for temp and the rest is condition
    let temperature = "N/A";
    let condition = "N/A";

    if (tempAndCondition.length >= 1) {
      temperature = tempAndCondition[0].trim();
    }
    if (tempAndCondition.length >= 2) {
      // Join the rest back in case condition has spaces
      condition = rest.substring(rest.indexOf(tempAndCondition[1])).trim();
    }

    log("--- Current Weather ---");
    log("Location: " + location);
    log("Temperature: " + temperature);
    log("Condition: " + condition);
    log("-----------------------");

  } else {
    log("Could not parse weather data: " + weatherData);
  }
}

function parseAndLogQuote(quoteJson: String): void {
  // Basic JSON parsing using string manipulation.
  // Example: {"content":"Quote text.","author":"Author Name"}
  let quote = "N/A";
  let author = "N/A";

  const contentMarker = '"content":"';
  const authorMarker = '"author":"';
  const endMarker = '"';

  let contentStartIndex = quoteJson.indexOf(contentMarker);
  if (contentStartIndex != -1) {
    contentStartIndex += contentMarker.length;
    const contentEndIndex = quoteJson.indexOf(endMarker, contentStartIndex);
    if (contentEndIndex != -1) {
      quote = quoteJson.substring(contentStartIndex, contentEndIndex);
    }
  }

  let authorStartIndex = quoteJson.indexOf(authorMarker);
  if (authorStartIndex != -1) {
    authorStartIndex += authorMarker.length;
    const authorEndIndex = quoteJson.indexOf(endMarker, authorStartIndex);
    if (authorEndIndex != -1) {
      author = quoteJson.substring(authorStartIndex, authorEndIndex);
    }
  }

  log("--- Quote of the Day ---");
  log('"' + quote + '"');
  log("- " + author);
  log("------------------------");
}

// handleBriefingError function is no longer used after removing try...catch blocks.
// Removing it.
/*
function handleBriefingError(context: string, url: string, e: unknown): void {
  let errorMessage = "Error during " + context + " (" + url + "): ";
  if (e instanceof BufferTooSmallError) {
     errorMessage += "Buffer too small. " + e.message + (e.code ? " Code: " + e.code.toString() : "");
  } else if (e instanceof HostFunctionError) {
    errorMessage += "Host function error. " + e.message + (e.code ? " Code: " + e.code.toString() : "");
  } else if (e instanceof Error) { // Catch generic AssemblyScript errors
    errorMessage += e.name + " - " + e.message;
  } else {
    // For 'unknown' type, we can't directly access properties.
    // We can try to convert it to a string if it's a primitive or has a toString method.
    // AssemblyScript's behavior with 'unknown' in catch is less defined than TypeScript's.
    // A simple approach is to log a generic message or try a basic string conversion.
    errorMessage += "Unknown error caught. Attempting to stringify: ";
    // @ts-ignore: Attempting toString on unknown, best effort.
    errorMessage += e ? e.toString() : "null/undefined";
  }
  log(errorMessage);
}
*/