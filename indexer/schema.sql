CREATE TABLE IF NOT EXISTS blocks (
    id SERIAL PRIMARY KEY,
    number BIGINT UNIQUE NOT NULL,
    hash VARCHAR(66) NOT NULL,
    parent_hash VARCHAR(66),
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    extrinsics_count INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS extrinsics (
    id SERIAL PRIMARY KEY,
    block_number BIGINT REFERENCES blocks(number),
    extrinsic_index INTEGER NOT NULL,
    hash VARCHAR(66),
    method VARCHAR(100),
    section VARCHAR(50),
    success BOOLEAN,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_blocks_number ON blocks(number);
CREATE INDEX IF NOT EXISTS idx_extrinsics_block ON extrinsics(block_number);