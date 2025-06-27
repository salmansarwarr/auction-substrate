module.exports = {
    // Update these with your Substrate node details
    wsEndpoint: 'ws://127.0.0.1:9944', // Local node
    // wsEndpoint: 'wss://your-substrate-node.com:443', // Remote node
    
    // Chain-specific configuration
    ss58Format: 42, // Generic Substrate format
    
    // Directories
    walletsDir: './wallets',
    
    // Default values
    defaultAmount: '1000000000000', // 1 token (assuming 12 decimals)
  };