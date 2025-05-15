# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Features

- async support
- SQLite support (Tests)
- Relational Mapping
- Uuid as a primary key in table


## [0.1.1] - 2025-05-15

### Features

- Postgresql support (Tested).
- UUID
- SQL Model

### Added

- v0.1.1 Model can now generate the queries from the Struct based on the model(table) attribute or from the name of the struct itself 

### Fixed

- Fixed Uuid table column generation logic
- Fixed insert query in case if a column type is set to Uuid in struct model attribute


[unreleased]: https://github.com/TarunVishwakarma1/rustix-orm/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/TarunVishwakarma1/rustix-orm/compare/v0.1.1...v0.1.0
