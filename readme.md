# Search Engine Crawler

This repository contains the crawler component for a fast, efficient, and open-source search engine built in Rust.

## Features

- Download crawled web pages of top `100,000` websites.
- Add relevant data to a PostgreSQL database.

## Getting Started

### Prerequisites

- Rust (latest stable version)
- PostgreSQL

### Installation

1. **Clone the repository:**

   ```sh
   git clone https://github.com/yvanlok/search_engine_crawler.git
   cd search_engine_crawler
   ```

2. **Install dependencies:**

   ```sh
   cargo build
   ```

3. **Set up the database:**

   ```sh
   psql -U postgres -f schema.sql
   ```

4. **Run the crawler:**
   ```sh
   cargo run
   ```

## Related Projects

- [Search Engine API](https://github.com/yvanlok/search_engine_api)
- [Search Engine UI](https://github.com/yvanlok/search-engine-ui)

## Contributing

1. Fork the repository.
2. Create your feature branch (`git checkout -b feature/new-feature`).
3. Commit your changes (`git commit -am 'Add new feature'`).
4. Push to the branch (`git push origin feature/new-feature`).
5. Create a new Pull Request.
