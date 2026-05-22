markdown_content = """# Transparent Volunteer Fund

A DAO-like escrow smart contract built on Stellar Soroban. This project aims to bring complete transparency and accountability to charitable donations and volunteer campaigns by leveraging blockchain technology for fund tracking, proposal-based withdrawals, and donor-driven voting mechanisms.

## 🌟 Key Features

- **Transparent Donations (`donate`):** Donors can contribute tokens safely to the campaign's escrow fund.
- **Proposal Creation (`create_proposal`):** The administrator can create detailed withdrawal proposals specifying the exact description, destination address, and fund amount required for volunteer operations.
- **Community Voting (`vote`):** Donors exercise governance rights by voting to approve or reject pending withdrawal proposals based on their contribution weight.
- **Secure Fund Disbursal (`execute_proposal`):** Funds are only released to the specified destination once a proposal achieves a democratic consensus threshold (e.g., >50% approval of total active votes/contributions).

## 🚀 Deployment Metadata

- **Network:** Stellar Testnet
- **Accepted Token:** XLM Testnet (`CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`)
- **Contract ID:** `YOUR_CONTRACT_ID_HERE` (Starts with `C...`)

## 🛠 Compilation & Interaction Guide (Soroban CLI)

Follow these command-line steps to build, deploy, initialize, and interact with the smart contract using the Soroban CLI.

### 1. Initialize a Local Admin Identity
Generate a secure local cryptographic identity named `admin` and fund it with Testnet XLM via Friendbot:

```bash
stellar keys generate admin --network testnet --fund
```
### 2. Compile the Smart Contract

Compile the Rust source code into an optimized WebAssembly (WASM) binary
:
```bash
stellar contract build
```
### 3. Deploy to Stellar Testnet

Upload and instantiate the compiled WASM binary onto the Testnet network using your funded admin identity:
Author Info
```bash
    stellar contract deploy \\
    --wasm target/wasm32v1-none/release/hello_world.wasm \\
    --source admin \\
    --network testnet
```
Note: This command will return your unique Contract ID (e.g., CD64YDLZ...). Copy this ID for the next steps.
### 4. Initialize the Campaign Quota
Configure the initial state of the contract by binding the administrator identity, assigning a campaign name, and mapping the accepted asset token:

``` bash
stellar contract invoke \\
  --id <YOUR_CONTRACT_ID> \\
  --source admin \\
  --network testnet \\
  -- initialize \\
  --admin admin \\
  --name "mua_he_xanh" \\
  --token CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC
```
### 5. Simulate a Donation Transaction
To contribute funds into the smart contract escrow program, invoke the donate function:
```bash
stellar contract invoke \\
  --id <YOUR_CONTRACT_ID> \\
  --source admin \\
  --network testnet \\
  -- donate \\
  --caller <DONOR_WALLET_ADDRESS> \\
  --amount 100000000
```

    Developer: Le Duc Thinh (Lê Đức Thịnh)
    Student ID: 24125045
    Institution: Faculty of Information Technology, VNU-HCM University of Science
