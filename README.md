# Clutch Hub API

![Alpha](https://img.shields.io/badge/status-alpha-orange.svg)
![Experimental](https://img.shields.io/badge/stage-experimental-red.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)

> ⚠️ **ALPHA SOFTWARE** - This project is in active development and is considered experimental. Use at your own risk. APIs may change without notice.

## Overview
Clutch Hub API is a backend service designed to manage and provide data for the Clutch Hub application. It handles various operations such as user authentication, data retrieval, and data manipulation.

**Created and maintained by [Mehran Mazhar](https://github.com/MehranMazhar)**

## Features
- User authentication and authorization
- CRUD operations for various resources
- Data validation and error handling
- Integration with external services
- Detailed API documentation
- Data modeling for resources
- Example requests and responses

## API Documentation

### Authentication

#### POST /auth/register
Registers a new user.

**Request Body:**
```json
{
  "username": "string",
  "password": "string",
  "email": "string"
}
```

**Response:**
```json
{
  "id": "string",
  "username": "string",
  "email": "string",
  "token": "string"
}
```

#### POST /auth/login
Logs in a user.

**Request Body:**
```json
{
  "username": "string",
  "password": "string"
}
```

**Response:**
```json
{
  "token": "string"
}
```

### Users

- `GET /users`: Retrieve a list of users.
- `POST /users`: Create a new user.
- `GET /users/{id}`: Retrieve a user by ID.
- `PUT /users/{id}`: Update a user by ID.
- `DELETE /users/{id}`: Delete a user by ID.

## Data Models

### User
- **id**: string
- **username**: string
- **email**: string
- **createdAt**: Date
- **updatedAt**: Date

## Installation

### Prerequisites
- Docker and Docker Compose (recommended)
- OR Rust (1.70+) and Cargo for local development

## 🐳 Docker Setup (Recommended)

### Full Stack (Node + API) - Use clutch-deploy
For the complete Clutch stack (node + API + monitoring), use the [clutch-deploy](https://github.com/clutchprotocol/clutch-deploy) repository:
```bash
git clone https://github.com/clutchprotocol/clutch-deploy.git
cd clutch-deploy
cp .env.example .env
docker-compose up -d
```

### API Only - Development
```bash
# Copy environment and start (builds from source)
cp env.example .env
docker-compose up --build
```

Or run a single container:
```bash
docker build -t clutch-hub-api .
docker run -p 3000:3000 --env-file .env clutch-hub-api
```

Pre-built image:
```bash
docker pull 9194010019/clutch-hub-api:latest
docker run -p 3000:3000 --env-file .env 9194010019/clutch-hub-api:latest
```

The API will be available at `http://localhost:3000`

### Development with Docker
```powershell
# Use the development script (Windows)
.\scripts\docker-dev.ps1 build    # Build image
.\scripts\docker-dev.ps1 up       # Start services
.\scripts\docker-dev.ps1 logs     # View logs
.\scripts\docker-dev.ps1 health   # Check health
.\scripts\docker-dev.ps1 clean    # Clean up
```

### Pre-built Docker Images
```bash
docker pull 9194010019/clutch-hub-api:latest
docker run -p 3000:3000 --env-file .env 9194010019/clutch-hub-api:latest
```

## 🦀 Local Rust Setup

### Prerequisites
- Rust (1.76+)
- Cargo

### Setup
1. Clone the repository:
    ```bash
    git clone https://github.com/MehranMazhar/clutch-hub-api.git
    ```
2. Navigate to the project directory:
    ```bash
    cd clutch-hub-api
    ```
3. Copy environment configuration:
    ```bash
    cp env.example .env
    ```
4. Build the project:
    ```bash
    cargo build --release
    ```

## Usage
1. Start the API server:
    ```bash
    cargo run
    ```
2. The API will be available at `http://localhost:3000` (or configured port).

Example API calls:
  - **Register a new user:**
    ```bash
    curl -X POST http://localhost:3000/auth/register \
    -H "Content-Type: application/json" \
    -d '{"username": "john_doe", "password": "securepassword", "email": "john@example.com"}'
    ```
  - **Login:**
    ```bash
    curl -X POST http://localhost:3000/auth/login \
    -H "Content-Type: application/json" \
    -d '{"username": "john_doe", "password": "securepassword"}'
    ```

## Configuration
- Update the `.env` file with your environment variables.

## Contributing
1. Fork the repository.
2. Create a new branch (`git checkout -b feature-branch`).
3. Make your changes.
4. Commit your changes (`git commit -m 'Add some feature'`).
5. Push to the branch (`git push origin feature-branch`).
6. Open a pull request.

## Author & Maintainer

**Mehran Mazhar**
- GitHub: [@MehranMazhar](https://github.com/MehranMazhar)
- Website: [MehranMazhar.com](https://MehranMazhar.com)
- Email: mehran.mazhar@gmail.com

## License
This project is licensed under the MIT License.