const { ApiPromise, WsProvider } = require('@polkadot/api');
const pool = require('./db');
require('dotenv').config();

class SubstrateIndexer {
  constructor() {
    this.api = null;
    this.isShuttingDown = false;
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = 5;
  }

  async initialize() {
    try {
      await this.initializeDatabase();
      await this.connectToNode();
      return true;
    } catch (error) {
      console.error('Failed to initialize:', error);
      return false;
    }
  }

  async connectToNode() {
    try {
      console.log(`Connecting to ${process.env.WS_ENDPOINT}...`);
      
      const wsProvider = new WsProvider(process.env.WS_ENDPOINT, 1000, {}, 10000);
      
      // Add connection event handlers
      wsProvider.on('connected', () => {
        console.log('✓ Connected to Substrate node');
        this.reconnectAttempts = 0;
      });

      wsProvider.on('disconnected', () => {
        console.log('✗ Disconnected from Substrate node');
        if (!this.isShuttingDown) {
          this.handleReconnection();
        }
      });

      wsProvider.on('error', (error) => {
        console.error('WebSocket error:', error.message);
      });

      this.api = await ApiPromise.create({ provider: wsProvider });
      
      console.log(`Chain: ${await this.api.rpc.system.chain()}`);
      console.log(`Node name: ${await this.api.rpc.system.name()}`);
      console.log(`Node version: ${await this.api.rpc.system.version()}`);
      
    } catch (error) {
      console.error('Connection error:', error.message);
      throw error;
    }
  }

  async handleReconnection() {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error('Max reconnection attempts reached. Exiting...');
      process.exit(1);
    }

    this.reconnectAttempts++;
    const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
    
    console.log(`Attempting to reconnect in ${delay/1000} seconds... (${this.reconnectAttempts}/${this.maxReconnectAttempts})`);
    
    setTimeout(async () => {
      try {
        await this.connectToNode();
        this.startIndexing();
      } catch (error) {
        console.error('Reconnection failed:', error.message);
        this.handleReconnection();
      }
    }, delay);
  }

  async initializeDatabase() {
    try {
      // Existing blocks table
      await pool.query(`
        CREATE TABLE IF NOT EXISTS blocks (
          id SERIAL PRIMARY KEY,
          number BIGINT UNIQUE NOT NULL,
          hash VARCHAR(66) NOT NULL,
          parent_hash VARCHAR(66),
          timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
          extrinsics_count INTEGER DEFAULT 0
        );
      `);

      // Auctions table
      await pool.query(`
        CREATE TABLE IF NOT EXISTS auctions (
          id SERIAL PRIMARY KEY,
          collection_id TEXT NOT NULL,
          item_id TEXT NOT NULL,
          owner_account TEXT NOT NULL,
          start_block BIGINT NOT NULL,
          highest_bid BIGINT DEFAULT 0,
          highest_bidder TEXT,
          ended BOOLEAN DEFAULT FALSE,
          block_number BIGINT NOT NULL,
          timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
          UNIQUE(collection_id, item_id, block_number)
        );
      `);

      // Bids table
      await pool.query(`
        CREATE TABLE IF NOT EXISTS bids (
          id SERIAL PRIMARY KEY,
          collection_id TEXT NOT NULL,
          item_id TEXT NOT NULL,
          bidder_account TEXT NOT NULL,
          bid_amount BIGINT NOT NULL,
          block_number BIGINT NOT NULL,
          timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
      `);

      // Auction status table
      await pool.query(`
        CREATE TABLE IF NOT EXISTS auction_status (
          id SERIAL PRIMARY KEY,
          collection_id TEXT NOT NULL,
          item_id TEXT NOT NULL,
          in_auction BOOLEAN NOT NULL,
          block_number BIGINT NOT NULL,
          timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
          UNIQUE(collection_id, item_id, block_number)
        );
      `);

      // Pallet settings table
      await pool.query(`
        CREATE TABLE IF NOT EXISTS pallet_settings (
          id SERIAL PRIMARY KEY,
          setting_name TEXT UNIQUE NOT NULL,
          setting_value TEXT NOT NULL,
          block_number BIGINT NOT NULL,
          timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
      `);
      
      console.log('Database schema initialized');
    } catch (error) {
      console.error('Database initialization error:', error);
      throw error;
    }
  }

  async fetchAuctionStorage(blockHash) {
      const palletName = 'template'; 
      
      // Fetch all auctions
      const auctionsEntries = await this.api.query[palletName].auctions.entriesAt(blockHash);
      
      for (const [key, value] of auctionsEntries) {
        const [collectionId, itemId] = key.args;
        const auctionInfo = value.toJSON();
        
        await pool.query(`
          INSERT INTO auctions (collection_id, item_id, owner_account, start_block, highest_bid, highest_bidder, ended, block_number)
          VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
          ON CONFLICT (collection_id, item_id, block_number) DO UPDATE SET
            owner_account = $3,
            start_block = $4,
            highest_bid = $5,
            highest_bidder = $6,
            ended = $7
        `, [
          collectionId,
          itemId || "",
          auctionInfo.owner,
          auctionInfo.startBlock,
          auctionInfo.highestBid,
          auctionInfo.highestBidder,
          auctionInfo.ended,
          await this.getBlockNumber(blockHash)
        ]);
      }

      // Fetch all bids
      const bidsEntries = await this.api.query[palletName].bids.entriesAt(blockHash);
      
      for (const [key, value] of bidsEntries) {
        const [collectionId, itemId] = key.args;
        const bidsArray = value.toJSON();
        
        // Clear existing bids for this auction at this block
        const blockNumber = await this.getBlockNumber(blockHash);
        await pool.query(
          'DELETE FROM bids WHERE collection_id = $1 AND item_id = $2 AND block_number = $3',
          [collectionId.toString(), itemId.toString(), blockNumber]
        );
        
        // Insert current bids
        for (const [bidder, amount] of bidsArray) {
          await pool.query(`
            INSERT INTO bids (collection_id, item_id, bidder_account, bid_amount, block_number)
            VALUES ($1, $2, $3, $4, $5)
          `, [
            collectionId.toString(),
            itemId.toString(),
            bidder,
            amount,
            blockNumber
          ]);
        }
      }

      // Fetch auction status
      const inAuctionEntries = await this.api.query[palletName].inAuction.entriesAt(blockHash);
      
      for (const [key, value] of inAuctionEntries) {
        const [collectionId, itemId] = key.args;
        const inAuction = value.toJSON();
        
        await pool.query(`
          INSERT INTO auction_status (collection_id, item_id, in_auction, block_number)
          VALUES ($1, $2, $3, $4)
          ON CONFLICT (collection_id, item_id, block_number) DO UPDATE SET
            in_auction = $3
        `, [
          collectionId,
          itemId || "",
          inAuction,
          await this.getBlockNumber(blockHash)
        ]);
      }

      // Fetch pallet settings
      const feePercentage = await this.api.query[palletName].feePercentage.at(blockHash);
      const accumulatedFees = await this.api.query[palletName].accumulatedFees.at(blockHash);
      
      const blockNumber = await this.getBlockNumber(blockHash);
      
      await pool.query(`
        INSERT INTO pallet_settings (setting_name, setting_value, block_number)
        VALUES ($1, $2, $3), ($4, $5, $6)
        ON CONFLICT (setting_name) DO UPDATE SET
          setting_value = EXCLUDED.setting_value,
          block_number = EXCLUDED.block_number
      `, [
        'fee_percentage',
        feePercentage.toString(),
        blockNumber,
        'accumulated_fees',
        accumulatedFees.toString(),
        blockNumber
      ]);
  }

  async getBlockNumber(blockHash) {
    if (typeof blockHash === 'string') {
      const header = await this.api.rpc.chain.getHeader(blockHash);
      return header.number.toNumber();
    }
    return blockHash.number.toNumber();
  }

  async indexBlock(header) {
    if (this.isShuttingDown) return;

    try {
      const blockNumber = header.number.toNumber();
      const hash = header.hash.toHex();
      const parentHash = header.parentHash.toHex();

      console.log(`Indexing block: #${blockNumber}`);

      // Index basic block info
      await pool.query(
        'INSERT INTO blocks (number, hash, parent_hash) VALUES ($1, $2, $3) ON CONFLICT (number) DO NOTHING',
        [blockNumber, hash, parentHash]
      );

      // Fetch and index pallet storage
      await this.fetchAuctionStorage(hash);

      console.log(`✓ Indexed block #${blockNumber} with storage data`);
      
    } catch (error) {
      console.error(`Error indexing block:`, error);
    }
  }

  // Method to fetch storage for a specific auction
  async getAuctionData(collectionId, itemId, blockHash = null) {
    try {
      const palletName = 'nftAuction'; // Update to your pallet name
      
      if (blockHash) {
        const auction = await this.api.query[palletName].auctions.at(blockHash, [collectionId, itemId]);
        const bids = await this.api.query[palletName].bids.at(blockHash, [collectionId, itemId]);
        const inAuction = await this.api.query[palletName].inAuction.at(blockHash, [collectionId, itemId]);
        
        return {
          auction: auction.toJSON(),
          bids: bids.toJSON(),
          inAuction: inAuction.toJSON()
        };
      } else {
        const auction = await this.api.query[palletName].auctions([collectionId, itemId]);
        const bids = await this.api.query[palletName].bids([collectionId, itemId]);
        const inAuction = await this.api.query[palletName].inAuction([collectionId, itemId]);
        
        return {
          auction: auction.toJSON(),
          bids: bids.toJSON(),
          inAuction: inAuction.toJSON()
        };
      }
    } catch (error) {
      console.error('Error fetching auction data:', error);
      return null;
    }
  }

  // Method to get all active auctions
  async getAllActiveAuctions() {
    try {
      const palletName = 'nftAuction'; // Update to your pallet name
      const auctions = await this.api.query[palletName].auctions.entries();
      
      const activeAuctions = [];
      for (const [key, value] of auctions) {
        const auctionInfo = value.toJSON();
        if (!auctionInfo.ended) {
          const [collectionId, itemId] = key.args;
          activeAuctions.push({
            collectionId: collectionId.toString(),
            itemId: itemId.toString(),
            ...auctionInfo
          });
        }
      }
      
      return activeAuctions;
    } catch (error) {
      console.error('Error fetching active auctions:', error);
      return [];
    }
  }

  async startIndexing() {
    try {
      // Subscribe to new blocks
      this.unsubscribe = await this.api.rpc.chain.subscribeNewHeads(
        (header) => this.indexBlock(header)
      );
      
      console.log('Started indexing new blocks...');
    } catch (error) {
      console.error('Failed to start indexing:', error);
      throw error;
    }
  }

  async start() {
    if (!await this.initialize()) {
      process.exit(1);
    }

    await this.startIndexing();

    // Graceful shutdown
    const shutdown = async () => {
      console.log('\nShutting down indexer...');
      this.isShuttingDown = true;
      
      if (this.unsubscribe) {
        this.unsubscribe();
      }
      
      if (this.api) {
        await this.api.disconnect();
      }
      
      await pool.end();
      process.exit(0);
    };

    process.on('SIGINT', shutdown);
    process.on('SIGTERM', shutdown);
    
    console.log('Substrate indexer started. Press Ctrl+C to stop.');
  }
}

// Start the indexer
const indexer = new SubstrateIndexer();
indexer.start().catch((error) => {
  console.error('Failed to start indexer:', error);
  process.exit(1);
});