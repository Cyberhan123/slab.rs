import { core } from "ext:core/mod.js";

const headers = core.loadExtScript("ext:deno_fetch/20_headers.js");
const formData = core.loadExtScript("ext:deno_fetch/21_formdata.js");
const httpClient = core.loadExtScript("ext:deno_fetch/22_http_client.js");
const request = core.loadExtScript("ext:deno_fetch/23_request.js");
const response = core.loadExtScript("ext:deno_fetch/23_response.js");
const fetch = core.loadExtScript("ext:deno_fetch/26_fetch.js");
const eventSource = core.loadExtScript("ext:deno_fetch/27_eventsource.js");

core.setWasmStreamingCallback(fetch.handleWasmStreaming);

import {applyToGlobal, writeable, nonEnumerable} from 'ext:rustyscript/rustyscript.js';

applyToGlobal({
    fetch: writeable(fetch.fetch),
    Request: nonEnumerable(request.Request),
    Response: nonEnumerable(response.Response),
    Headers: nonEnumerable(headers.Headers),
    FormData: nonEnumerable(formData.FormData),
    EventSource: nonEnumerable(eventSource.EventSource)
});

globalThis.Deno.HttpClient = httpClient.HttpClient;
globalThis.Deno.createHttpClient = httpClient.createHttpClient;
