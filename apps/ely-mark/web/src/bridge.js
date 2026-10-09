/** One request to the app, as a promise. */
export function ask(request) {
  return new Promise((resolve, reject) => {
    window.elyQuery({
      request: JSON.stringify(request),
      onSuccess: resolve,
      onFailure: (code, message) => reject(new Error(`${code}: ${message}`)),
    })
  })
}
