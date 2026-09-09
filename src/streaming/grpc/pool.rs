use super::types::{AccountPretty, BlockMetaPretty, TransactionPretty};
use std::collections::VecDeque;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use yellowstone_grpc_proto::{
    geyser::{SubscribeUpdateAccount, SubscribeUpdateBlockMeta, SubscribeUpdateTransaction},
    prost_types::Timestamp,
};

/// 通用对象池特征
pub trait ObjectPool<T> {
    fn acquire(&self) -> PooledObject<T>;
    fn return_object(&self, obj: Box<T>);
}

/// 带自动归还的智能指针
pub struct PooledObject<T> {
    object: Option<Box<T>>,
    pool: Arc<Mutex<VecDeque<Box<T>>>>,
    max_size: usize,
}

impl<T> PooledObject<T> {
    #[allow(dead_code)]
    fn new(object: Box<T>, pool: Arc<Mutex<VecDeque<Box<T>>>>, max_size: usize) -> Self {
        Self { object: Some(object), pool, max_size }
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(obj) = self.object.take() {
            let mut pool = self.pool.lock().unwrap();
            if pool.len() < self.max_size {
                pool.push_back(obj);
            }
            // 超过最大容量时直接丢弃
        }
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.object.as_ref().unwrap()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut().unwrap()
    }
}

/// AccountPretty 对象池
pub struct AccountPrettyPool {
    pool: Arc<Mutex<VecDeque<Box<AccountPretty>>>>,
    max_size: usize,
}

impl AccountPrettyPool {
    pub fn new(initial_size: usize, max_size: usize) -> Self {
        let mut pool = VecDeque::with_capacity(initial_size);

        // 预分配对象
        for _ in 0..initial_size {
            pool.push_back(Box::new(AccountPretty::default()));
        }

        Self { pool: Arc::new(Mutex::new(pool)), max_size }
    }

    pub fn acquire(&self) -> PooledAccountPretty {
        let mut pool = self.pool.lock().unwrap();
        let account = match pool.pop_front() {
            Some(reused) => reused,
            None => Box::new(AccountPretty::default()),
        };

        PooledAccountPretty {
            account: Some(account),
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
        }
    }
}

/// 带自动归还的 AccountPretty
pub struct PooledAccountPretty {
    account: Option<Box<AccountPretty>>,
    pool: Arc<Mutex<VecDeque<Box<AccountPretty>>>>,
    max_size: usize,
}

impl PooledAccountPretty {
    /// 从 gRPC 更新重置数据
    pub fn reset_from_update(&mut self, account_update: SubscribeUpdateAccount) {
        *self.deref_mut() = account_update.into();
    }
}

impl Drop for PooledAccountPretty {
    fn drop(&mut self) {
        let mut pool = self.pool.lock().unwrap();
        if pool.len() < self.max_size {
            let account = self.account.as_mut().expect("pooled object");
            account.data.clear();
            account.signature = Default::default();
            account.pubkey = Default::default();
            account.owner = Default::default();
            pool.push_back(self.account.take().expect("pooled object"));
        }
    }
}

impl std::ops::Deref for PooledAccountPretty {
    type Target = AccountPretty;

    fn deref(&self) -> &Self::Target {
        self.account.as_deref().expect("pooled object")
    }
}

impl std::ops::DerefMut for PooledAccountPretty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.account.as_deref_mut().expect("pooled object")
    }
}

/// BlockMetaPretty 对象池
pub struct BlockMetaPrettyPool {
    pool: Arc<Mutex<VecDeque<Box<BlockMetaPretty>>>>,
    max_size: usize,
}

impl BlockMetaPrettyPool {
    pub fn new(initial_size: usize, max_size: usize) -> Self {
        let mut pool = VecDeque::with_capacity(initial_size);

        // 预分配对象
        for _ in 0..initial_size {
            pool.push_back(Box::new(BlockMetaPretty::default()));
        }

        Self { pool: Arc::new(Mutex::new(pool)), max_size }
    }

    pub fn acquire(&self) -> PooledBlockMetaPretty {
        let mut pool = self.pool.lock().unwrap();
        let block_meta = match pool.pop_front() {
            Some(reused) => reused,
            None => Box::new(BlockMetaPretty::default()),
        };

        PooledBlockMetaPretty {
            block_meta: Some(block_meta),
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
        }
    }
}

/// 带自动归还的 BlockMetaPretty
pub struct PooledBlockMetaPretty {
    block_meta: Option<Box<BlockMetaPretty>>,
    pool: Arc<Mutex<VecDeque<Box<BlockMetaPretty>>>>,
    max_size: usize,
}

impl PooledBlockMetaPretty {
    /// 从 gRPC 更新重置数据
    pub fn reset_from_update(
        &mut self,
        block_update: SubscribeUpdateBlockMeta,
        block_time: Option<Timestamp>,
    ) {
        *self.deref_mut() = (block_update, block_time).into();
    }
}

impl Drop for PooledBlockMetaPretty {
    fn drop(&mut self) {
        let mut pool = self.pool.lock().unwrap();
        if pool.len() < self.max_size {
            let block_meta = self.block_meta.as_mut().expect("pooled object");
            block_meta.block_hash.clear();
            block_meta.block_time = None;
            pool.push_back(self.block_meta.take().expect("pooled object"));
        }
    }
}

impl std::ops::Deref for PooledBlockMetaPretty {
    type Target = BlockMetaPretty;

    fn deref(&self) -> &Self::Target {
        self.block_meta.as_deref().expect("pooled object")
    }
}

impl std::ops::DerefMut for PooledBlockMetaPretty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.block_meta.as_deref_mut().expect("pooled object")
    }
}

/// TransactionPretty 对象池
pub struct TransactionPrettyPool {
    pool: Arc<Mutex<VecDeque<Box<TransactionPretty>>>>,
    max_size: usize,
}

impl TransactionPrettyPool {
    pub fn new(initial_size: usize, max_size: usize) -> Self {
        let mut pool = VecDeque::with_capacity(initial_size);

        // 预分配对象
        for _ in 0..initial_size {
            pool.push_back(Box::new(TransactionPretty::default()));
        }

        Self { pool: Arc::new(Mutex::new(pool)), max_size }
    }

    pub fn acquire(&self) -> PooledTransactionPretty {
        let mut pool = self.pool.lock().unwrap();
        let transaction = match pool.pop_front() {
            Some(reused) => reused,
            None => Box::new(TransactionPretty::default()),
        };

        PooledTransactionPretty {
            transaction: Some(transaction),
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
        }
    }
}

/// 带自动归还的 TransactionPretty
pub struct PooledTransactionPretty {
    transaction: Option<Box<TransactionPretty>>,
    pool: Arc<Mutex<VecDeque<Box<TransactionPretty>>>>,
    max_size: usize,
}

impl PooledTransactionPretty {
    /// 从 gRPC 更新重置数据
    pub fn reset_from_update(
        &mut self,
        tx_update: SubscribeUpdateTransaction,
        block_time: Option<Timestamp>,
    ) {
        *self.deref_mut() = (tx_update, block_time).into();
    }
}

impl Drop for PooledTransactionPretty {
    fn drop(&mut self) {
        let mut pool = self.pool.lock().unwrap();
        if pool.len() < self.max_size {
            let transaction = self.transaction.as_mut().expect("pooled object");
            transaction.grpc_tx = Default::default();
            transaction.block_hash.clear();
            transaction.block_time = None;
            transaction.signature = Default::default();
            pool.push_back(self.transaction.take().expect("pooled object"));
        }
    }
}

impl std::ops::Deref for PooledTransactionPretty {
    type Target = TransactionPretty;

    fn deref(&self) -> &Self::Target {
        self.transaction.as_deref().expect("pooled object")
    }
}

impl std::ops::DerefMut for PooledTransactionPretty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.transaction.as_deref_mut().expect("pooled object")
    }
}

/// EventPretty 对象池（组合池）
pub struct EventPrettyPool {
    account_pool: AccountPrettyPool,
    block_pool: BlockMetaPrettyPool,
    transaction_pool: TransactionPrettyPool,
}

impl EventPrettyPool {
    pub fn new() -> Self {
        Self {
            account_pool: AccountPrettyPool::new(10000, 20000),
            block_pool: BlockMetaPrettyPool::new(500, 1000),
            transaction_pool: TransactionPrettyPool::new(10000, 20000),
        }
    }

    /// 获取账户事件对象
    pub fn acquire_account(&self) -> PooledAccountPretty {
        self.account_pool.acquire()
    }

    /// 获取区块事件对象
    pub fn acquire_block(&self) -> PooledBlockMetaPretty {
        self.block_pool.acquire()
    }

    /// 获取交易事件对象
    pub fn acquire_transaction(&self) -> PooledTransactionPretty {
        self.transaction_pool.acquire()
    }
}

/// 对象池管理器（单例）
pub struct PoolManager {
    event_pool: EventPrettyPool,
}

impl PoolManager {
    pub fn new() -> Self {
        Self { event_pool: EventPrettyPool::new() }
    }

    pub fn get_event_pool(&self) -> &EventPrettyPool {
        &self.event_pool
    }
}

impl Default for PoolManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 工厂函数用于创建优化的 EventPretty
impl EventPrettyPool {
    /// 直接移动账户数据，保留原有工厂接口。
    pub fn create_account_event_optimized(&self, update: SubscribeUpdateAccount) -> AccountPretty {
        update.into()
    }

    /// 直接移动区块数据，保留原有工厂接口。
    pub fn create_block_event_optimized(
        &self,
        update: SubscribeUpdateBlockMeta,
        block_time: Option<Timestamp>,
    ) -> BlockMetaPretty {
        (update, block_time).into()
    }

    /// 直接移动交易数据，保留原有工厂接口。
    pub fn create_transaction_event_optimized(
        &self,
        update: SubscribeUpdateTransaction,
        block_time: Option<Timestamp>,
    ) -> TransactionPretty {
        (update, block_time).into()
    }
}

// 全局池管理器实例
lazy_static::lazy_static! {
    pub static ref GLOBAL_POOL_MANAGER: PoolManager = PoolManager::new();
}

/// 便捷的全局工厂函数
pub mod factory {
    use super::*;

    /// Compatibility factory: moves the account payload without acquiring a pool.
    pub fn create_account_pretty_pooled(update: SubscribeUpdateAccount) -> AccountPretty {
        update.into()
    }

    /// Compatibility factory: moves the block payload without acquiring a pool.
    pub fn create_block_meta_pretty_pooled(
        update: SubscribeUpdateBlockMeta,
        block_time: Option<Timestamp>,
    ) -> BlockMetaPretty {
        (update, block_time).into()
    }

    /// Compatibility factory: moves the transaction payload without acquiring a pool.
    pub fn create_transaction_pretty_pooled(
        update: SubscribeUpdateTransaction,
        block_time: Option<Timestamp>,
    ) -> TransactionPretty {
        (update, block_time).into()
    }
}
