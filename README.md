<!-- To make diffing easier, this file leans towards using semantic linebreaks ( https://sembr.org/ ) -->

# TMR Client

Thomas's client for the [Montrose MCP](https://www.montrose.io/mcp) API.

The software does not contain any AI functionality.

This project is not affiliated, endorsed by and does not have any connections to Montrose. 

## API Stability

The available APIs of the Montrose MCP service has been fetched by asking the service for its available tools.
Since the API has no versioning and AI agents should be able to adopt to changing APIs,
one should not expect the API to be stable.
This means that TMR client can stop working at any moment.

## Example Client

An example client is available in the `examples` directory.
It can be run as follows:

```bash
cargo run --example simple-client
```

## Security Warning

The MCP API does not permit any actions taken on your behalf,
but a malicious actor can extract information about your Montrose account and investments.
**You should never trust third party code, such as the code in this repository,
without examing the source code carefully.**

To re-iterate from the license:

> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## License

This software is licensed under the [MIT license](LICENSE).