// Minimal URLPattern implementation for web_stub
// This is a simplified version that may not support all features

class URLPattern {
  #pattern;
  #baseURL;

  constructor(input, baseURL) {
    if (typeof input === 'string') {
      this.#pattern = input;
      this.#baseURL = baseURL;
    } else if (typeof input === 'object' && input !== null) {
      this.#pattern = input;
      this.#baseURL = baseURL;
    } else {
      throw new TypeError('Invalid URLPattern input');
    }
  }

  test(input, baseURL) {
    try {
      this.exec(input, baseURL);
      return true;
    } catch {
      return false;
    }
  }

  exec(input, baseURL) {
    // Simplified implementation - just check if URL is valid
    let url;
    if (typeof input === 'string') {
      url = new URL(input, baseURL);
    } else if (input instanceof URL) {
      url = input;
    } else {
      return null;
    }

    // Return a basic result structure
    return {
      inputs: [input],
      protocol: { input: url.protocol.slice(0, -1), groups: {} },
      username: { input: url.username, groups: {} },
      password: { input: url.password, groups: {} },
      hostname: { input: url.hostname, groups: {} },
      port: { input: url.port, groups: {} },
      pathname: { input: url.pathname, groups: {} },
      search: { input: url.search.slice(1), groups: {} },
      hash: { input: url.hash.slice(1), groups: {} },
    };
  }

  get protocol() { return this.#pattern?.protocol ?? '*'; }
  get username() { return this.#pattern?.username ?? '*'; }
  get password() { return this.#pattern?.password ?? '*'; }
  get hostname() { return this.#pattern?.hostname ?? '*'; }
  get port() { return this.#pattern?.port ?? '*'; }
  get pathname() { return this.#pattern?.pathname ?? '*'; }
  get search() { return this.#pattern?.search ?? '*'; }
  get hash() { return this.#pattern?.hash ?? '*'; }
}

export { URLPattern };
