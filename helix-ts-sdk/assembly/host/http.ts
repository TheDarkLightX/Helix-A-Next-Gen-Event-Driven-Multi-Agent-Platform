// @ts-ignore: decorator
@external("env", "host_http_request_open")
declare function wasm_host_http_request_open(method_ptr: usize, method_len: usize, url_ptr: usize, url_len: usize): u32;

// @ts-ignore: decorator
@external("env", "host_http_request_set_header")
declare function wasm_host_http_request_set_header(request_handle: u32, name_ptr: usize, name_len: usize, value_ptr: usize, value_len: usize): i32;

// @ts-ignore: decorator
@external("env", "host_http_request_send")
declare function wasm_host_http_request_send(request_handle: u32, body_ptr: usize, body_len: usize, timeout_ms: u32): u32;

// @ts-ignore: decorator
@external("env", "host_http_response_get_status_code")
declare function wasm_host_http_response_get_status_code(response_handle: u32): u32;

// @ts-ignore: decorator
@external("env", "host_http_response_get_header_len")
declare function wasm_host_http_response_get_header_len(response_handle: u32, name_ptr: usize, name_len: usize): i32;

// @ts-ignore: decorator
@external("env", "host_http_response_get_header_value")
declare function wasm_host_http_response_get_header_value(response_handle: u32, name_ptr: usize, name_len: usize, buf_ptr: usize, buf_len: usize): i32;

// @ts-ignore: decorator
@external("env", "host_http_response_get_body_len")
declare function wasm_host_http_response_get_body_len(response_handle: u32): i32;

// @ts-ignore: decorator
@external("env", "host_http_response_get_body")
declare function wasm_host_http_response_get_body(response_handle: u32, buf_ptr: usize, buf_len: usize): i32;

// @ts-ignore: decorator
@external("env", "host_http_request_close")
declare function wasm_host_http_request_close(request_handle: u32): void;

// @ts-ignore: decorator
@external("env", "host_http_response_close")
declare function wasm_host_http_response_close(response_handle: u32): void;

import {
  writeStringToSharedBuffer,
  readStringFromSharedBuffer,
  getSharedBufferPtr,
  getSharedBufferSize,
  // We'll need a way to read raw bytes from shared buffer for response body
} from "../utils/memory"; // .ts extension removed
import {
  HostFunctionError,
  BufferTooSmallError,
  ValueNotFoundError, // May not be directly used here but good for consistency
  WasmHostErrorCode,
} from "../utils/errors"; // .ts extension removed
import { logMessage as log } from "./log"; // Added import for log

export class HttpHeader {
  constructor(public name: string, public value: string) {}
}

export class HttpResponse {
  constructor(
    public status: u32,
    public headers: Array<HttpHeader>, // For simplicity, header fetching is not fully implemented in this refactor
    public body: ArrayBuffer | null // Raw body
  ) {}

  getHeader(name: string): string | null {
    // Simplified: full header parsing from host would be complex with current shared buffer.
    // This would require iterating headers via host calls.
    const lowerName = name.toLowerCase();
    for (let i = 0; i < this.headers.length; i++) {
      if (this.headers[i].name.toLowerCase() == lowerName) {
        return this.headers[i].value;
      }
    }
    return null;
  }

  text(): String | null {
    const body = this.body;
    if (body !== null) {
      // Assuming UTF-8. Error handling for decode needed if not guaranteed.
      return String.UTF8.decode(body);
    }
    return null;
  }

  // json<T>(): T | null { ... } // Requires JSON parser
}

export class HttpRequest {
  private request_handle: u32 = 0;
  private url: string;
  private method: string;
  private headers_to_set: Array<HttpHeader> = [];
  private req_body_bytes: ArrayBuffer | null = null;

  constructor(method: string, url: string) {
    this.method = method;
    this.url = url;
  }

  setHeader(name: string, value: string): HttpRequest {
    this.headers_to_set.push(new HttpHeader(name, value));
    return this;
  }

  body(data: ArrayBuffer): HttpRequest { // Changed to ArrayBuffer only
    this.req_body_bytes = data;
    return this;
  }

  // Overload for string convenience, if needed by many agents,
  // or agents can do String.UTF8.encode themselves.
  // For now, keeping it simple with ArrayBuffer only in the core method.
  /*
  bodyString(data: string): HttpRequest {
    this.req_body_bytes = String.UTF8.encode(data);
    return this;
  }
  */

  send(timeout_ms: u32 = 5000): HttpResponse | null {
    // If try...catch is not supported, errors from writeStringToSharedBuffer (which uses assert) will halt execution.
    // This simplifies the logic here as we don't need to catch and re-throw.
    // The "used before assigned" errors should also disappear if asserts halt on failure.

    const methodLen = writeStringToSharedBuffer(this.method);
    const methodPtr = getSharedBufferPtr();
    // Host must copy method before we write URL
    const urlLen = writeStringToSharedBuffer(this.url);
    const urlPtr = getSharedBufferPtr(); // URL is now at start of shared buffer

    // Open request
    // Host must read URL from urlPtr/urlLen
    this.request_handle = wasm_host_http_request_open(methodPtr, methodLen, urlPtr, urlLen);
    if (this.request_handle == 0) {
      // Cannot throw if exceptions are not supported. Return null or an error indicator.
      // For now, let's assume if request_handle is 0, subsequent calls will fail gracefully or return error codes.
      // Or, the agent should check this. For simplicity, we'll proceed and let host calls fail.
      // A more robust solution would be to return a custom error object or null and have agent check.
      log("SDK: Failed to open HTTP request with host. Request handle is 0.");
      return null;
    }

    // Set headers
    for (let i = 0; i < this.headers_to_set.length; i++) {
      const h = this.headers_to_set[i];
      const headerNameLen = writeStringToSharedBuffer(h.name);
      const headerNamePtr = getSharedBufferPtr();
      // Host must copy header name
      const headerValueLen = writeStringToSharedBuffer(h.value);
      const headerValuePtr = getSharedBufferPtr(); // Value at start of shared buffer

      // Host must read header value
      const headerSetResult = wasm_host_http_request_set_header(
        this.request_handle, headerNamePtr, headerNameLen, headerValuePtr, headerValueLen
      );
      if (headerSetResult !== 0) { // Assuming 0 is success
        wasm_host_http_request_close(this.request_handle);
        log(`SDK: Host failed to set HTTP header '${h.name}'. Code: ${headerSetResult}`);
        return null; // Indicate error
      }
    }

    // Send request
    let bodyPtr: usize = 0;
    let bodyLen: usize = 0;
    if (this.req_body_bytes) {
      if (this.req_body_bytes!.byteLength > getSharedBufferSize()) {
          wasm_host_http_request_close(this.request_handle);
          log("SDK: HTTP request body too large for shared buffer.");
          // Ideally, throw new BufferTooSmallError, but if exceptions are out:
          return null;
      }
      // Manually copy ArrayBuffer to shared buffer
      const bodyBytesView = Uint8Array.wrap(this.req_body_bytes!);
      const destPtr = getSharedBufferPtr();
      for (let i: i32 = 0; i < bodyBytesView.length; i++) {
          store<u8>(destPtr + i, bodyBytesView[i]);
      }
      bodyPtr = destPtr;
      bodyLen = bodyBytesView.byteLength;
    }

    const response_handle = wasm_host_http_request_send(this.request_handle, bodyPtr, bodyLen, timeout_ms);
    wasm_host_http_request_close(this.request_handle); // Close request handle
    this.request_handle = 0;

    if (response_handle == 0) {
      return null; // Send failed or timed out
    }

    // Process response
    const status_code = wasm_host_http_response_get_status_code(response_handle);
    const response_headers_arr: Array<HttpHeader> = []; // Simplified: header fetching needs more work

    let response_body_bytes: ArrayBuffer | null = null;
    const res_body_len = wasm_host_http_response_get_body_len(response_handle);

    if (res_body_len > 0) {
      if (res_body_len > getSharedBufferSize()) {
        wasm_host_http_response_close(response_handle);
        throw new BufferTooSmallError(`HTTP response body (size: ${res_body_len}) too large for shared buffer (size: ${getSharedBufferSize()}).`);
      }
      const res_buf_ptr = getSharedBufferPtr(); // Host writes body to shared buffer
      const bytes_read = wasm_host_http_response_get_body(response_handle, res_buf_ptr, res_body_len);

      if (bytes_read == res_body_len) {
        response_body_bytes = new ArrayBuffer(res_body_len);
        // Manually copy from shared buffer to new ArrayBuffer
        const targetBodyView = Uint8Array.wrap(response_body_bytes); // Removed ! as it's assigned above
        const sourcePtr = res_buf_ptr;
        for (let i: i32 = 0; i < res_body_len; i++) {
            store<u8>(changetype<usize>(targetBodyView.dataStart) + i, load<u8>(sourcePtr + i));
        }
      } else if (bytes_read < 0) {
         log(`SDK: Error reading HTTP response body. Code: ${bytes_read}`);
      }
    } else if (res_body_len == 0) {
      response_body_bytes = new ArrayBuffer(0);
    }
    // Ensure response_body_bytes is non-null if res_body_len was > 0 but read failed.
    // Or, the HttpResponse constructor needs to handle potentially null body if bytes_read indicated error.
    // For now, if bytes_read < 0 and res_body_len > 0, response_body_bytes will be null.

    wasm_host_http_response_close(response_handle);
    return new HttpResponse(status_code, response_headers_arr, response_body_bytes);
  }
}

// Convenience functions (httpGet, httpPost) would use the HttpRequest class.
// These should now also avoid throwing exceptions if the main send() doesn't.
// They will return null on failure.

export function httpGet(url: string, headers: Array<HttpHeader> | null = null, timeout_ms: u32 = 5000): HttpResponse | null {
    const request = new HttpRequest("GET", url);
    if (headers) {
        for (let i = 0; i < headers.length; i++) {
            request.setHeader(headers[i].name, headers[i].value);
        }
    }
    // Errors inside send() will now result in null return or assert.
    return request.send(timeout_ms);
}

export function httpPost(url: string, bodyBuffer: ArrayBuffer, headers: Array<HttpHeader> | null = null, timeout_ms: u32 = 5000): HttpResponse | null {
    const request = new HttpRequest("POST", url);
    if (headers) {
        for (let i = 0; i < headers.length; i++) {
            request.setHeader(headers[i].name, headers[i].value);
        }
    }
    request.body(bodyBuffer);
    // Errors inside send() will now result in null return or assert.
    return request.send(timeout_ms);
}
