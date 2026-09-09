# Order Book Programming Exercise

Welcome to the non-crossing matching engine coding challenge! 🦑

Implement a [limit order book](https://www.investopedia.com/terms/l/limitorderbook.asp) system that maintains price-time
priority order books for trading symbols. The program should accept new orders, cancellations, and flushes from stdin
and publish acknowledgements, rejections, and top-of-book changes to stdout.

## How We Will Test Your Submission

Your solution will be evaluated using these commands:

```bash
docker build -t non-crossing-matching-engine .
docker run --rm non-crossing-matching-engine
```

**Requirements:**

- Must produce a binary named `non_crossing_matching_engine`
- Must read CSV commands from **stdin**
- Must write CSV output to **stdout**
- **All 12 test cases must pass** (non-negotiable)

💡 **Note**: Please don't modify the test files - we'll be running your solution against additional test cases in the
same format.

## Requirements

### Input - stdin

Your program reads [CSV](https://en.wikipedia.org/wiki/Comma-separated_values) commands from **stdin**. Three command
types are supported:

**New Order:**

```csv
N, userId, symbol, price, quantity, side, userOrderId
```

**Cancel Order:**

```csv
C, userId, userOrderId
```

**Flush (reset all order books):**

```csv
F
```

### Order Book Logic

The order book uses **price-time priority**:

- Orders join their respective side ([bid or ask](https://www.investopedia.com/terms/b/bid-and-ask.asp)) in price
  priority, then time priority
- **REJECT** orders that would [cross the book](https://en.wikipedia.org/wiki/Order_book#Crossed_book) (don't match
  them)
    - A buy order crosses if its price >= best ask
    - A sell order crosses if its price <= best bid

### Output - stdout

Your program publishes to **stdout** in CSV format:

**Order acknowledgement:**

```csv
A, userId, userOrderId
```

**Order rejection (would cross the book):**

```csv
R, userId, userOrderId
```

**Top of book change:**

```csv
B, side (B or S), price, totalQuantity
```

- Use `-` for price and totalQuantity when a side has no orders

### Test Cases

The test suite includes 12 scenarios covering:

- Balanced book operations
- Shallow bid/ask handling
- Order rejections (crossing prevention)
- Spread management
- Cancellation edge cases
- Volume changes at top of book
- Flush command

All test cases include both input and expected output files.

## Build Requirements

Your submission must:

- Produce a binary named `non_crossing_matching_engine`
- Read from stdin
- Write to stdout
- Work in the provided Docker environment (Ubuntu 24.04, x64)

## Testing

Run tests using Docker (works on all platforms):

```bash
# Build and run tests (reports saved to ./reports/)
docker build -t non-crossing-matching-engine .
docker run --rm -v "$(pwd)/reports:/reports" non-crossing-matching-engine
```

Test results are displayed in the terminal and saved to `./reports/`.

## Submission

Your submission should include:

1. Repository root - Source code and build configuration
2. **test/** folder - All test cases (provided)
3. **README.md** - With some useful info for us, see below.

### Creating the Submission Bundle

Once you've completed the implementation:

1. **Ensure ALL tests pass** - Run the Docker tests to verify. All 12 test cases must pass.
2. Verify your submission contains:
    - Root-level source code and build configuration
    - `test/` folder with all test cases
3. Create a git bundle:
   ```bash
   git bundle create orderbook_<yourname>.bundle master
   ```
4. Send the bundle file to your recruiter

### Bonus (Optional - but we love this stuff!)

If time allows, enhance your README.md with:

- **Project structure**: Document architectural aspects (threads, classes, modules)
- **Time and space complexity**: Describe the [time complexity](https://en.wikipedia.org/wiki/Time_complexity)
  and [space complexity](https://en.wikipedia.org/wiki/Space_complexity) of processing new orders and cancellations
- **Improvements**: What you would do differently with more time

We enjoy reading about your design thinking, so don't be shy!

## Important Notes

- ✅ **All test cases must pass** - This is the foundation we build on
- 💼 Please don't submit proprietary code or code from other companies
- 🚫 Orders that would cross the book must be **rejected**, not matched
- 📦 Your bundle should contain the source, `test/`, and `README.md`
- 🧪 Test thoroughly before submitting - the Docker environment is your friend!

---

**Good luck! We're excited to see your solution.** 🚀

This problem reflects the kind of systems we build at non-crossing matching engine. Take your time, test thoroughly, and show us how you think
about order book design!
