import { logMessage as log } from "../host/log";
import {
  HttpRequest,
  HttpResponse
} from "../host/http";
// HttpError, TimeoutError, WasmHttpError are not standard exports.
// We'll rely on HostFunctionError and BufferTooSmallError from utils.
import { WasmHostErrorCode, HostFunctionError, BufferTooSmallError } from "../utils/errors";


export function agent_main(): void {
  log("HTTP example agent started.");

  // --- Test GET request ---
  const getUrl = "https://httpbin.org/get";
  log("Attempting GET request to: " + getUrl);

  let getRequest = new HttpRequest("GET", getUrl);
  let getResponse: HttpResponse | null = null;

  try {
    // Body is not set for GET, send takes only timeout.
    getResponse = getRequest.send(5000); // 5 second timeout

    if (getResponse) {
      log("GET request successful. Status: " + getResponse.status.toString());

      const contentType = getResponse.getHeader("content-type");
      if (contentType) {
        log("GET Response Content-Type: " + contentType);
      } else {
        log("GET Response Content-Type header not found.");
      }

      const bodyLength = getResponse.body ? getResponse.body!.byteLength : 0;
      log("GET Response Body Length: " + bodyLength.toString());

      if (bodyLength > 0) {
        const bodyText = getResponse.text();
        if (bodyText !== null) {
          log("GET Response Body: " + bodyText);
        } else {
          log("GET Response Body: Failed to retrieve body text despite positive length.");
        }
      } else if (bodyLength == 0) {
        log("GET Response Body: Empty (0 length).");
      }
      // HttpResponse.close() is not needed, handled by HttpRequest.send()
    } else {
      log("GET request failed to return a response object (returned null).");
    }
  } catch (e: any) {
    handleHttpError("GET " + getUrl, e);
  }
  // HttpRequest.close() is not needed, handled by HttpRequest.send()

  // --- Test POST request ---
  const postUrl = "https://httpbin.org/post";
  const postBodyString = '{"message": "Hello from Helix Agent!", "timestamp": ' + Date.now().toString() + '}';
  log("Attempting POST request to: " + postUrl + " with body: " + postBodyString);

  let postRequest = new HttpRequest("POST", postUrl);
  postRequest.setHeader("Content-Type", "application/json");
  postRequest.setHeader("X-Custom-Header", "Helix-POST-Test");
  const postBodyBuffer = String.UTF8.encode(postBodyString);
  postRequest.body(postBodyBuffer); // Set body using the body() method

  let postResponse: HttpResponse | null = null;

  try {
    postResponse = postRequest.send(5000); // 5 second timeout

    if (postResponse) {
      log("POST request successful. Status: " + postResponse.status.toString());

      const responseBodyText = postResponse.text();
      if (responseBodyText !== null) {
        log("POST Response Body: " + responseBodyText);
      } else {
        log("POST Response Body: Failed to retrieve body text.");
      }
      // HttpResponse.close() is not needed
    } else {
      log("POST request failed to return a response object (returned null).");
    }
  } catch (e: any) {
    handleHttpError("POST " + postUrl, e);
  }
  // HttpRequest.close() is not needed

  log("HTTP example agent finished.");
}

function handleHttpError(context: string, e: any): void {
  if (e instanceof BufferTooSmallError) {
     log("HTTP Error (" + context + "): Buffer too small. " + e.message + (e.code ? " Code: " + e.code.toString() : ""));
  } else if (e instanceof HostFunctionError) {
    log("HTTP Error (" + context + "): Host function error. " + e.message + (e.code ? " Code: " + e.code.toString() : ""));
  } else if (e instanceof Error) { // Catch generic AssemblyScript errors
    log("Unexpected Error (" + context + "): " + e.name + " - " + e.message);
  } else {
    log("Unknown Error (" + context + "): " + e.toString());
  }
}