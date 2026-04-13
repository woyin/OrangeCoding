//! # 加密存储模块
//!
//! 使用 AES-256-GCM 对敏感信息（如 API 密钥）进行加密存储。
//! 通过 PBKDF2 从用户口令派生加密密钥，确保密钥安全。
//! 加密后的数据存储在本地文件中。

use std::path::PathBuf;

use ring::aead::{self, Aad, BoundKey, Nonce, NonceSequence, NONCE_LEN};
use ring::error::Unspecified;
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use chengcoding_core::CeairError;

/// PBKDF2 迭代次数，确保足够的抗暴力破解强度
const PBKDF2_ITERATIONS: u32 = 100_000;

/// 派生密钥长度（32 字节 = 256 位，用于 AES-256）
const KEY_LEN: usize = 32;

/// 盐值长度（16 字节）
const SALT_LEN: usize = 16;

// ---------------------------------------------------------------------------
// Nonce 序列实现
// ---------------------------------------------------------------------------

/// 用于 AES-GCM 加密的 Nonce 序列
///
/// 每次加密操作使用一个随机生成的 Nonce，防止重放攻击。
struct OneNonceSequence(Option<aead::Nonce>);

impl OneNonceSequence {
    /// 从字节数组创建 Nonce 序列
    fn new(nonce_bytes: [u8; NONCE_LEN]) -> Self {
        Self(Some(aead::Nonce::assume_unique_for_key(nonce_bytes)))
    }
}

impl NonceSequence for OneNonceSequence {
    fn advance(&mut self) -> Result<Nonce, Unspecified> {
        self.0.take().ok_or(Unspecified)
    }
}

// ---------------------------------------------------------------------------
// 加密数据的存储格式
// ---------------------------------------------------------------------------

/// 加密后的数据包，包含解密所需的所有信息
#[derive(Debug, Serialize, Deserialize)]
struct EncryptedData {
    /// PBKDF2 使用的盐值（Base64 编码）
    salt: Vec<u8>,

    /// AES-GCM 使用的随机数（Base64 编码）
    nonce: Vec<u8>,

    /// 加密后的密文（Base64 编码，包含认证标签）
    ciphertext: Vec<u8>,
}

/// 密钥存储文件的顶层结构
#[derive(Debug, Serialize, Deserialize, Default)]
struct SecretStore {
    /// 键值对映射：名称 -> 加密数据
    secrets: std::collections::HashMap<String, EncryptedData>,
}

// ---------------------------------------------------------------------------
// 加密存储器
// ---------------------------------------------------------------------------

/// 加密存储器，管理敏感信息的加密与解密
///
/// 使用 AES-256-GCM 进行对称加密，密钥通过 PBKDF2 从口令派生。
pub struct CryptoStore {
    /// 存储文件路径
    store_path: PathBuf,

    /// 安全随机数生成器
    rng: SystemRandom,
}

impl CryptoStore {
    /// 创建新的加密存储器
    ///
    /// `store_path` 指定加密数据的存储文件位置。
    pub fn new(store_path: PathBuf) -> Self {
        Self {
            store_path,
            rng: SystemRandom::new(),
        }
    }

    /// 从默认配置目录创建加密存储器
    ///
    /// 存储文件位于 ~/.config/chenagent/secrets.json
    pub fn default_store() -> chengcoding_core::Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| CeairError::config("无法确定用户主目录"))?;
        let store_path = home.join(".config").join("chenagent").join("secrets.json");
        Ok(Self::new(store_path))
    }

    /// 使用 PBKDF2 从口令派生 AES-256 密钥
    ///
    /// 使用 HMAC-SHA256 作为伪随机函数，迭代 100,000 次。
    fn derive_key(&self, passphrase: &str, salt: &[u8]) -> [u8; KEY_LEN] {
        let mut key = [0u8; KEY_LEN];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(PBKDF2_ITERATIONS).unwrap(),
            salt,
            passphrase.as_bytes(),
            &mut key,
        );
        key
    }

    /// 加密给定的明文值
    ///
    /// 返回包含盐值、Nonce 和密文的加密数据包。
    pub fn encrypt_value(
        &self,
        plaintext: &str,
        passphrase: &str,
    ) -> chengcoding_core::Result<Vec<u8>> {
        debug!("开始加密数据");

        // 生成随机盐值
        let mut salt = [0u8; SALT_LEN];
        self.rng
            .fill(&mut salt)
            .map_err(|_| CeairError::internal("生成随机盐值失败"))?;

        // 生成随机 Nonce
        let mut nonce_bytes = [0u8; NONCE_LEN];
        self.rng
            .fill(&mut nonce_bytes)
            .map_err(|_| CeairError::internal("生成随机 Nonce 失败"))?;

        // 从口令派生密钥
        let key_bytes = self.derive_key(passphrase, &salt);

        // 创建 AES-256-GCM 密钥和加密器
        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &key_bytes)
            .map_err(|_| CeairError::internal("创建加密密钥失败"))?;
        let nonce_seq = OneNonceSequence::new(nonce_bytes);
        let mut sealing_key = aead::SealingKey::new(unbound_key, nonce_seq);

        // 执行加密（就地加密，附加认证标签）
        let mut in_out = plaintext.as_bytes().to_vec();
        sealing_key
            .seal_in_place_append_tag(Aad::empty(), &mut in_out)
            .map_err(|_| CeairError::internal("加密操作失败"))?;

        // 组装加密数据包并序列化
        let encrypted = EncryptedData {
            salt: salt.to_vec(),
            nonce: nonce_bytes.to_vec(),
            ciphertext: in_out,
        };

        serde_json::to_vec(&encrypted)
            .map_err(|e| CeairError::serialization(format!("加密数据序列化失败: {e}")))
    }

    /// 解密加密后的数据
    ///
    /// 使用相同的口令和存储的盐值重新派生密钥进行解密。
    pub fn decrypt_value(
        &self,
        encrypted_bytes: &[u8],
        passphrase: &str,
    ) -> chengcoding_core::Result<String> {
        debug!("开始解密数据");

        // 反序列化加密数据包
        let encrypted: EncryptedData = serde_json::from_slice(encrypted_bytes)
            .map_err(|e| CeairError::serialization(format!("加密数据反序列化失败: {e}")))?;

        // 从口令派生密钥（使用存储的盐值）
        let key_bytes = self.derive_key(passphrase, &encrypted.salt);

        // 恢复 Nonce
        let nonce_bytes: [u8; NONCE_LEN] = encrypted
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| CeairError::internal("Nonce 长度无效"))?;

        // 创建解密器
        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &key_bytes)
            .map_err(|_| CeairError::internal("创建解密密钥失败"))?;
        let nonce_seq = OneNonceSequence::new(nonce_bytes);
        let mut opening_key = aead::OpeningKey::new(unbound_key, nonce_seq);

        // 执行解密
        let mut ciphertext = encrypted.ciphertext;
        let plaintext = opening_key
            .open_in_place(Aad::empty(), &mut ciphertext)
            .map_err(|_| CeairError::internal("解密失败，口令可能不正确"))?;

        String::from_utf8(plaintext.to_vec())
            .map_err(|e| CeairError::internal(format!("解密结果不是有效的 UTF-8: {e}")))
    }

    /// 将密钥存储到文件
    ///
    /// 使用口令加密后存入本地存储文件。如果同名密钥已存在，则覆盖。
    pub fn store_secret(
        &self,
        name: &str,
        value: &str,
        passphrase: &str,
    ) -> chengcoding_core::Result<()> {
        info!("存储密钥: {}", name);

        // 加载现有存储（如果存在）
        let mut store = self.load_store()?;

        // 加密值
        let encrypted = self.encrypt_to_data(value, passphrase)?;

        // 插入或更新
        store.secrets.insert(name.to_string(), encrypted);

        // 保存到文件
        self.save_store(&store)?;

        info!("密钥 '{}' 已安全存储", name);
        Ok(())
    }

    /// 从文件中检索并解密密钥
    pub fn retrieve_secret(
        &self,
        name: &str,
        passphrase: &str,
    ) -> chengcoding_core::Result<String> {
        debug!("检索密钥: {}", name);

        let store = self.load_store()?;

        let encrypted = store
            .secrets
            .get(name)
            .ok_or_else(|| CeairError::config(format!("密钥 '{}' 不存在", name)))?;

        // 解密
        self.decrypt_from_data(encrypted, passphrase)
    }

    /// 加密值并返回加密数据结构（内部使用）
    fn encrypt_to_data(
        &self,
        plaintext: &str,
        passphrase: &str,
    ) -> chengcoding_core::Result<EncryptedData> {
        // 生成随机盐值
        let mut salt = [0u8; SALT_LEN];
        self.rng
            .fill(&mut salt)
            .map_err(|_| CeairError::internal("生成随机盐值失败"))?;

        // 生成随机 Nonce
        let mut nonce_bytes = [0u8; NONCE_LEN];
        self.rng
            .fill(&mut nonce_bytes)
            .map_err(|_| CeairError::internal("生成随机 Nonce 失败"))?;

        // 从口令派生密钥
        let key_bytes = self.derive_key(passphrase, &salt);

        // 加密
        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &key_bytes)
            .map_err(|_| CeairError::internal("创建加密密钥失败"))?;
        let nonce_seq = OneNonceSequence::new(nonce_bytes);
        let mut sealing_key = aead::SealingKey::new(unbound_key, nonce_seq);

        let mut in_out = plaintext.as_bytes().to_vec();
        sealing_key
            .seal_in_place_append_tag(Aad::empty(), &mut in_out)
            .map_err(|_| CeairError::internal("加密操作失败"))?;

        Ok(EncryptedData {
            salt: salt.to_vec(),
            nonce: nonce_bytes.to_vec(),
            ciphertext: in_out,
        })
    }

    /// 从加密数据结构解密（内部使用）
    fn decrypt_from_data(
        &self,
        encrypted: &EncryptedData,
        passphrase: &str,
    ) -> chengcoding_core::Result<String> {
        let key_bytes = self.derive_key(passphrase, &encrypted.salt);

        let nonce_bytes: [u8; NONCE_LEN] = encrypted
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| CeairError::internal("Nonce 长度无效"))?;

        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &key_bytes)
            .map_err(|_| CeairError::internal("创建解密密钥失败"))?;
        let nonce_seq = OneNonceSequence::new(nonce_bytes);
        let mut opening_key = aead::OpeningKey::new(unbound_key, nonce_seq);

        let mut ciphertext = encrypted.ciphertext.clone();
        let plaintext = opening_key
            .open_in_place(Aad::empty(), &mut ciphertext)
            .map_err(|_| CeairError::internal("解密失败，口令可能不正确"))?;

        String::from_utf8(plaintext.to_vec())
            .map_err(|e| CeairError::internal(format!("解密结果不是有效的 UTF-8: {e}")))
    }

    /// 从文件加载密钥存储
    fn load_store(&self) -> chengcoding_core::Result<SecretStore> {
        if self.store_path.exists() {
            let content =
                std::fs::read_to_string(&self.store_path).map_err(|e| CeairError::from(e))?;
            serde_json::from_str(&content)
                .map_err(|e| CeairError::serialization(format!("密钥存储文件解析失败: {e}")))
        } else {
            // 文件不存在时返回空存储
            Ok(SecretStore::default())
        }
    }

    /// 将密钥存储保存到文件
    fn save_store(&self, store: &SecretStore) -> chengcoding_core::Result<()> {
        // 确保父目录存在
        if let Some(parent) = self.store_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| CeairError::io(format!("创建密钥存储目录失败: {e}")))?;
            }
        }

        let content = serde_json::to_string_pretty(store)
            .map_err(|e| CeairError::serialization(format!("密钥存储序列化失败: {e}")))?;

        std::fs::write(&self.store_path, content).map_err(|e| CeairError::from(e))?;

        Ok(())
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试加密和解密的基本往返流程
    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let store = CryptoStore::new(tmp.path().join("secrets.json"));

        let plaintext = "sk-test-api-key-12345";
        let passphrase = "my-secure-passphrase";

        // 加密
        let encrypted = store
            .encrypt_value(plaintext, passphrase)
            .expect("加密失败");

        // 验证加密后的数据与原文不同
        assert_ne!(encrypted, plaintext.as_bytes());

        // 解密
        let decrypted = store
            .decrypt_value(&encrypted, passphrase)
            .expect("解密失败");

        // 验证解密后与原文一致
        assert_eq!(decrypted, plaintext);
    }

    /// 测试使用错误口令解密应失败
    #[test]
    fn test_decrypt_with_wrong_passphrase() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let store = CryptoStore::new(tmp.path().join("secrets.json"));

        let plaintext = "sensitive-data";
        let correct_passphrase = "correct-passphrase";
        let wrong_passphrase = "wrong-passphrase";

        let encrypted = store
            .encrypt_value(plaintext, correct_passphrase)
            .expect("加密失败");

        // 使用错误口令解密应该失败
        let result = store.decrypt_value(&encrypted, wrong_passphrase);
        assert!(result.is_err(), "使用错误口令应该解密失败");
    }

    /// 测试密钥的存储和检索
    #[test]
    fn test_store_and_retrieve_secret() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let store = CryptoStore::new(tmp.path().join("secrets.json"));

        let name = "openai_api_key";
        let value = "sk-abcdef123456";
        let passphrase = "master-password";

        // 存储密钥
        store
            .store_secret(name, value, passphrase)
            .expect("存储密钥失败");

        // 验证存储文件已创建
        assert!(tmp.path().join("secrets.json").exists());

        // 检索密钥
        let retrieved = store
            .retrieve_secret(name, passphrase)
            .expect("检索密钥失败");
        assert_eq!(retrieved, value);
    }

    /// 测试检索不存在的密钥应失败
    #[test]
    fn test_retrieve_nonexistent_secret() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let store = CryptoStore::new(tmp.path().join("secrets.json"));

        let result = store.retrieve_secret("nonexistent", "passphrase");
        assert!(result.is_err(), "检索不存在的密钥应该失败");
    }

    /// 测试存储多个密钥
    #[test]
    fn test_store_multiple_secrets() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let store = CryptoStore::new(tmp.path().join("secrets.json"));

        let passphrase = "master-password";

        // 存储多个密钥
        store
            .store_secret("key1", "value1", passphrase)
            .expect("存储密钥1失败");
        store
            .store_secret("key2", "value2", passphrase)
            .expect("存储密钥2失败");
        store
            .store_secret("key3", "value3", passphrase)
            .expect("存储密钥3失败");

        // 验证每个密钥都能正确检索
        assert_eq!(store.retrieve_secret("key1", passphrase).unwrap(), "value1");
        assert_eq!(store.retrieve_secret("key2", passphrase).unwrap(), "value2");
        assert_eq!(store.retrieve_secret("key3", passphrase).unwrap(), "value3");
    }

    /// 测试覆盖已存在的密钥
    #[test]
    fn test_overwrite_secret() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let store = CryptoStore::new(tmp.path().join("secrets.json"));

        let passphrase = "master-password";

        // 存储初始值
        store
            .store_secret("api_key", "old-value", passphrase)
            .expect("存储初始密钥失败");

        // 覆盖为新值
        store
            .store_secret("api_key", "new-value", passphrase)
            .expect("覆盖密钥失败");

        // 验证获取到的是新值
        let retrieved = store
            .retrieve_secret("api_key", passphrase)
            .expect("检索密钥失败");
        assert_eq!(retrieved, "new-value");
    }

    /// 测试加密中文和特殊字符
    #[test]
    fn test_encrypt_unicode() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let store = CryptoStore::new(tmp.path().join("secrets.json"));

        let plaintext = "这是一个包含中文和特殊字符的密钥 🔑！";
        let passphrase = "口令也可以是中文";

        let encrypted = store
            .encrypt_value(plaintext, passphrase)
            .expect("加密失败");
        let decrypted = store
            .decrypt_value(&encrypted, passphrase)
            .expect("解密失败");

        assert_eq!(decrypted, plaintext);
    }
}
