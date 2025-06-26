import { WsProvider, ApiPromise } from '@polkadot/api';
import {web3Enable, web3Accounts, web3FromAddress} from '@polkadot/extension-dapp';

class NFTAuctionApp {
    constructor() {
        this.api = null;
        this.account = null;
        this.accounts = [];
        this.auctions = [];
        this.wsProvider = null;
        this.injector = null;
        this.init();
    }

    async init() {
        this.setupEventListeners();
        await this.initializeApi();
        await this.checkExtension();
    }

    setupEventListeners() {
        document.getElementById('connectBtn').addEventListener('click', () => this.connectWallet());
        document.getElementById('listNftBtn').addEventListener('click', () => this.showListNftModal());
        document.getElementById('refreshBtn').addEventListener('click', () => this.loadAuctions());
        document.getElementById('closeModal').addEventListener('click', () => this.hideListNftModal());
        document.getElementById('listNftForm').addEventListener('submit', (e) => this.handleListNft(e));
        
        // Close modal when clicking outside
        document.getElementById('listNftModal').addEventListener('click', (e) => {
            if (e.target === document.getElementById('listNftModal')) {
                this.hideListNftModal();
            }
        });
    }

    async initializeApi() {
        try {
            // Connect to your Substrate node
            const WS_ENDPOINT = 'ws://127.0.0.1:9944'; 
            
            this.wsProvider = new WsProvider(WS_ENDPOINT);
            this.api = await ApiPromise.create({ 
                provider: this.wsProvider,
                types: {
                    // Add custom types if needed for your pallet
                    AuctionInfo: {
                        owner: 'AccountId',
                        start_block: 'BlockNumber',
                        highest_bid: 'Balance',
                        highest_bidder: 'Option<AccountId>',
                        ended: 'bool'
                    }
                }
            });

            console.log('Connected to Substrate node');
            
            // Listen for new blocks to update auction times
            this.api.rpc.chain.subscribeNewHeads((header) => {
                this.currentBlock = header.number.toNumber();
            });

        } catch (error) {
            console.error('Failed to connect to Substrate node:', error);
            this.showConnectionError();
        }
    }

    async checkExtension() {        
        // Check if Polkadot extension is installed
        const extensions = await web3Enable('NFT Auction House');
        if (extensions.length === 0) {
            this.showExtensionError();
            return;
        }

        console.log('Polkadot extension detected');
    }

    showConnectionError() {
        const container = document.getElementById('auctionsContainer');
        container.innerHTML = `
            <div class="empty-state">
                <h3>⚠️ Connection Error</h3>
                <p>Failed to connect to the Substrate node. Please ensure your node is running on ws://127.0.0.1:9944</p>
                <button class="btn" onclick="location.reload()">Retry Connection</button>
            </div>
        `;
    }

    showExtensionError() {
        const container = document.getElementById('auctionsContainer');
        container.innerHTML = `
            <div class="empty-state">
                <h3>🔌 Extension Required</h3>
                <p>Please install the Polkadot{.js} extension to use this app.</p>
                <a href="https://polkadot.js.org/extension/" target="_blank" class="btn">
                    Install Extension
                </a>
            </div>
        `;
    }

    async connectWallet() {
        if (this.account) {
            // Disconnect
            this.account = null;
            this.accounts = [];
            this.updateConnectionStatus(false);
            document.getElementById('listNftBtn').disabled = true;
            return;
        }

        try {            
            // Enable the extension
            const extensions = await web3Enable('NFT Auction House');
            if (extensions.length === 0) {
                throw new Error('No extension found');
            }

            // Get all accounts
            this.accounts = await web3Accounts();
            if (this.accounts.length === 0) {
                throw new Error('No accounts found');
            }

            // Use the first account for simplicity
            // In a real app, you might want to show account selection
            this.account = this.accounts[0];
            this.injector = await web3FromAddress(this.account.address);

            this.updateConnectionStatus(true);
            document.getElementById('listNftBtn').disabled = false;
            this.loadAuctions();

        } catch (error) {
            console.error('Failed to connect wallet:', error);
            alert('Failed to connect wallet. Please ensure Polkadot{.js} extension is installed and has accounts.');
        }
    }

    updateConnectionStatus(connected) {
        const statusEl = document.getElementById('connectionStatus');
        const connectBtn = document.getElementById('connectBtn');
        
        if (connected) {
            statusEl.className = 'connection-status connected';
            statusEl.textContent = `Connected: ${this.account.meta.name || 'Account'}`;
            connectBtn.textContent = 'Disconnect';
        } else {
            statusEl.className = 'connection-status disconnected';
            statusEl.textContent = 'Wallet Disconnected';
            connectBtn.textContent = 'Connect Wallet';
        }
    }

    showListNftModal() {
        document.getElementById('listNftModal').style.display = 'block';
    }

    hideListNftModal() {
        document.getElementById('listNftModal').style.display = 'none';
        document.getElementById('listNftForm').reset();
    }

    async handleListNft(e) {
        e.preventDefault();
        
        if (!this.api || !this.account) {
            alert('Please connect your wallet first');
            return;
        }

        const collectionId = document.getElementById('collectionId').value;
        const itemId = document.getElementById('itemId').value;

        try {
            // Create the extrinsic
            const tx = this.api.tx.template.listNftForAuction(collectionId, itemId);

            // Sign and send transaction
            const hash = await tx.signAndSend(this.account.address, { 
                signer: this.injector.signer 
            }, (status) => {
                if (status.isInBlock) {
                    console.log(`Transaction included at blockHash ${status.asInBlock}`);
                } else if (status.isFinalized) {
                    console.log(`Transaction finalized at blockHash ${status.asFinalized}`);
                    alert(`Successfully listed NFT ${collectionId}:${itemId} for auction!`);
                    this.hideListNftModal();
                    this.loadAuctions();
                }
            });
        } catch (error) {
            console.error('Failed to list NFT:', error);
            alert(`Failed to list NFT: ${error.message}`);
        }
    }

    async loadAuctions() {
        if (!this.api) {
            console.log('API not ready');
            return;
        }

        const container = document.getElementById('auctionsContainer');
        container.innerHTML = '<div class="loading"><h3>Loading auctions...</h3></div>';

        try {
            // Query all auctions from the blockchain
            const auctionEntries = await this.api.query.template.auctions.entries();
            const auctions = [];

            for (const [key, value] of auctionEntries) {
                if (value.isSome) {
                    const auctionInfo = value.unwrap();
                    const [collectionId, itemId] = key.args;
                    
                    // Calculate time left
                    const startBlock = auctionInfo.start_block;
                    const timeoutBlocks = this.api.consts.template.auctionTimeoutBlocks;
                    const currentBlock = this.currentBlock || 0;
                    const blocksLeft = Math.max(0, (startBlock + timeoutBlocks) - currentBlock);
                    const timeLeft = this.calculateTimeLeft(blocksLeft);

                    auctions.push({
                        collectionId: collectionId,
                        itemId: itemId,
                        owner: auctionInfo.owner,
                        startBlock: startBlock,
                        highestBid: auctionInfo.highestBid,
                        highestBidder: auctionInfo.highestBidder ? 
                            auctionInfo.highestBidder.unwrap() : null,
                        ended: auctionInfo.ended.toHuman(),
                        timeLeft: timeLeft,
                        blocksLeft: blocksLeft
                    });
                }
            }

            this.renderAuctions(auctions);
        } catch (error) {
            console.error('Failed to load auctions:', error);
            container.innerHTML = `
                <div class="empty-state">
                    <h3>Failed to load auctions</h3>
                    <p>Error: ${error.message}</p>
                    <button class="btn" onclick="app.loadAuctions()">Try Again</button>
                </div>
            `;
        }
    }

    calculateTimeLeft(blocksLeft) {
        if (blocksLeft <= 0) return 'Ended';
        
        // Assuming 6 seconds per block (adjust based on your chain)
        const secondsLeft = blocksLeft * 6;
        const hours = Math.floor(secondsLeft / 3600);
        const minutes = Math.floor((secondsLeft % 3600) / 60);
        
        if (hours > 0) {
            return `${hours}h ${minutes}m`;
        } else if (minutes > 0) {
            return `${minutes}m`;
        } else {
            return `${Math.floor(secondsLeft)}s`;
        }
    }

    renderAuctions(auctions) {
        const container = document.getElementById('auctionsContainer');
        
        if (auctions.length === 0) {
            container.innerHTML = `
                <div class="empty-state">
                    <h3>No active auctions</h3>
                    <p>Be the first to list an NFT for auction!</p>
                </div>
            `;
            return;
        }

        container.innerHTML = auctions.map(auction => `
            <div class="auction-card">
                <div class="auction-header">
                    <h3>NFT ${auction.collectionId}:${auction.itemId}</h3>
                    <span class="status-badge ${auction.ended ? 'status-ended' : 'status-active'}">
                        ${auction.ended ? 'Ended' : 'Active'}
                    </span>
                </div>
                <div class="auction-info">
                    <div class="info-row">
                        <span class="label">Owner:</span>
                        <span class="value">${this.formatAddress(auction.owner)}</span>
                    </div>
                    <div class="info-row">
                        <span class="label">Current Bid:</span>
                        <span class="value highest-bid">
                            ${auction.highestBid && auction.highestBid !== '0' ? 
                                this.formatBalance(auction.highestBid) + ' UNIT' : 'No bids'}
                        </span>
                    </div>
                    <div class="info-row">
                        <span class="label">Highest Bidder:</span>
                        <span class="value">
                            ${auction.highestBidder ? this.formatAddress(auction.highestBidder) : 'None'}
                        </span>
                    </div>
                </div>
                ${!auction.ended && this.account && auction.owner !== this.account.address ? `
                    <div class="auction-actions">
                        <input type="number" class="bid-input" placeholder="Enter bid amount" 
                               min="${auction.highestBid && auction.highestBid !== '0' ? 
                                   (parseFloat(this.formatBalance(auction.highestBid)) + 0.1) : 0.1}" 
                               step="0.1">
                        <button class="btn btn-success" onclick="app.placeBid(${auction.collectionId}, ${auction.itemId})">
                            Place Bid
                        </button>
                    </div>
                ` : ''}
                ${!auction.ended && this.account && auction.owner === this.account.address ? `
                    <div class="auction-actions">
                        <button class="btn btn-primary" onclick="app.resolveAuction(${auction.collectionId}, ${auction.itemId})">
                            Resolve Auction
                        </button>
                    </div>
                ` : ''}
            </div>
        `).join('');
    }

    async placeBid(collectionId, itemId) {
        if (!this.api || !this.account) {
            alert('Please connect your wallet first');
            return;
        }

        const card = event.target.closest('.auction-card');
        const bidInput = card.querySelector('.bid-input');
        const bidAmount = bidInput.value;

        if (!bidAmount || parseFloat(bidAmount) <= 0) {
            alert('Please enter a valid bid amount');
            return;
        }

        
        try {
            // Convert bid amount to blockchain units (assuming 12 decimals)
            const bidAmountUnits = this.api.createType('Balance', 
                parseFloat(bidAmount) * Math.pow(10, 12));
                
                console.log(collectionId[0], collectionId[1], bidAmountUnits)
            // Create the extrinsic
            const tx = this.api.tx.template.placeBid(collectionId[0], collectionId[1], bidAmountUnits);

            // Sign and send transaction
            await tx.signAndSend(this.account.address, { 
                signer: this.injector.signer 
            }, (status) => {
                if (status.isInBlock) {
                    console.log(`Bid transaction included at blockHash ${status.asInBlock}`);
                } else if (status.isFinalized) {
                    console.log(`Bid transaction finalized at blockHash ${status.asFinalized}`);
                    alert(`Successfully placed bid of ${bidAmount} UNIT on NFT ${collectionId[0]}:${collectionId[1]}!`);
                    bidInput.value = '';
                    this.loadAuctions();
                }
            });
        } catch (error) {
            console.error('Failed to place bid:', error);
            alert(`Failed to place bid: ${error.message}`);
        }
    }

    async resolveAuction(collectionId, itemId) {
        if (!this.api || !this.account) {
            alert('Please connect your wallet first');
            return;
        }

        try {
            // Create the extrinsic
            const tx = this.api.tx.nftAuction.resolveAuction(collectionId, itemId);

            // Sign and send transaction
            const hash = await tx.signAndSend(this.account.address, { 
                signer: this.injector.signer 
            }, (status) => {
                if (status.isInBlock) {
                    console.log(`Resolve transaction included at blockHash ${status.asInBlock}`);
                } else if (status.isFinalized) {
                    console.log(`Resolve transaction finalized at blockHash ${status.asFinalized}`);
                    alert(`Successfully resolved auction for NFT ${collectionId}:${itemId}!`);
                    this.loadAuctions();
                }
            });

            console.log('Resolve transaction hash:', hash.toHex());

        } catch (error) {
            console.error('Failed to resolve auction:', error);
            alert(`Failed to resolve auction: ${error.message}`);
        }
    }

    formatAddress(address) {
        // return `${address.substring(0, 6)}...${address.substring(address.length - 4)}`;
        return address;
    }

    formatBalance(balance) {
        // Convert from smallest unit to main unit (assuming 12 decimal places)
        const balanceNumber = typeof balance === 'string' ? 
            parseFloat(balance) : balance.toNumber();
        return (balanceNumber / Math.pow(10, 12)).toFixed(4);
    }
}

// Initialize the app
const app = new NFTAuctionApp();
window.app = app;