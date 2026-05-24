(function () {
class DOMException extends Error {
  constructor(message = "", name = "Error") {
    super(message);
    this.name = name;
  }

  get code() {
    return 0;
  }
}

return {
  DOMException,
  DOMExceptionPrototype: DOMException.prototype,
};
})();
