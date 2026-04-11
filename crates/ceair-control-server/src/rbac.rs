use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 用户角色
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// 可查看会话
    Viewer,
    /// 可发送消息、取消、审批低风险
    Operator,
    /// 可管理 worker、策略、令牌
    Admin,
}

/// 权限操作
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    SessionView,
    SessionCreate,
    SessionSendMessage,
    SessionCancel,
    SessionClose,
    ApprovalView,
    ApprovalRespond,
    WorkerView,
    WorkerManage,
    WorkerRevoke,
    AdminSettings,
}

/// 用户身份
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIdentity {
    pub user_id: String,
    pub username: String,
    pub roles: Vec<Role>,
    pub authenticated_at: chrono::DateTime<chrono::Utc>,
}

/// RBAC 策略引擎：根据角色→权限映射进行鉴权
pub struct RbacEngine {
    role_permissions: HashMap<Role, HashSet<Permission>>,
}

impl RbacEngine {
    /// 初始化默认角色→权限映射
    pub fn new() -> Self {
        let mut role_permissions = HashMap::new();

        // Viewer: 只读权限
        let viewer_perms: HashSet<Permission> = [
            Permission::SessionView,
            Permission::ApprovalView,
            Permission::WorkerView,
        ]
        .into_iter()
        .collect();

        // Operator: Viewer 权限 + 操作权限
        let mut operator_perms = viewer_perms.clone();
        operator_perms.insert(Permission::SessionCreate);
        operator_perms.insert(Permission::SessionSendMessage);
        operator_perms.insert(Permission::SessionCancel);
        operator_perms.insert(Permission::SessionClose);
        operator_perms.insert(Permission::ApprovalRespond);

        // Admin: Operator 权限 + 管理权限
        let mut admin_perms = operator_perms.clone();
        admin_perms.insert(Permission::WorkerManage);
        admin_perms.insert(Permission::WorkerRevoke);
        admin_perms.insert(Permission::AdminSettings);

        role_permissions.insert(Role::Viewer, viewer_perms);
        role_permissions.insert(Role::Operator, operator_perms);
        role_permissions.insert(Role::Admin, admin_perms);

        Self { role_permissions }
    }

    /// 检查用户是否拥有指定权限（遍历所有角色取并集）
    pub fn check(&self, identity: &UserIdentity, permission: &Permission) -> bool {
        identity.roles.iter().any(|role| {
            self.role_permissions
                .get(role)
                .map_or(false, |perms| perms.contains(permission))
        })
    }

    /// 检查用户是否拥有指定角色
    pub fn has_role(identity: &UserIdentity, role: &Role) -> bool {
        identity.roles.contains(role)
    }

    /// 获取角色拥有的所有权限
    pub fn get_permissions(&self, role: &Role) -> Vec<Permission> {
        self.role_permissions
            .get(role)
            .map(|perms| perms.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl Default for RbacEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_identity(roles: Vec<Role>) -> UserIdentity {
        UserIdentity {
            user_id: "u1".to_string(),
            username: "testuser".to_string(),
            roles,
            authenticated_at: Utc::now(),
        }
    }

    #[test]
    fn viewer_can_view() {
        let engine = RbacEngine::new();
        let identity = make_identity(vec![Role::Viewer]);
        assert!(engine.check(&identity, &Permission::SessionView));
        assert!(engine.check(&identity, &Permission::ApprovalView));
        assert!(engine.check(&identity, &Permission::WorkerView));
    }

    #[test]
    fn viewer_cannot_create() {
        let engine = RbacEngine::new();
        let identity = make_identity(vec![Role::Viewer]);
        assert!(!engine.check(&identity, &Permission::SessionCreate));
        assert!(!engine.check(&identity, &Permission::WorkerManage));
        assert!(!engine.check(&identity, &Permission::AdminSettings));
    }

    #[test]
    fn operator_can_create_and_cancel() {
        let engine = RbacEngine::new();
        let identity = make_identity(vec![Role::Operator]);
        assert!(engine.check(&identity, &Permission::SessionCreate));
        assert!(engine.check(&identity, &Permission::SessionCancel));
        assert!(engine.check(&identity, &Permission::SessionClose));
        assert!(engine.check(&identity, &Permission::ApprovalRespond));
        // Operator 不能管理 worker
        assert!(!engine.check(&identity, &Permission::WorkerManage));
    }

    #[test]
    fn admin_has_all_permissions() {
        let engine = RbacEngine::new();
        let identity = make_identity(vec![Role::Admin]);
        let all_permissions = vec![
            Permission::SessionView,
            Permission::SessionCreate,
            Permission::SessionSendMessage,
            Permission::SessionCancel,
            Permission::SessionClose,
            Permission::ApprovalView,
            Permission::ApprovalRespond,
            Permission::WorkerView,
            Permission::WorkerManage,
            Permission::WorkerRevoke,
            Permission::AdminSettings,
        ];
        for perm in &all_permissions {
            assert!(engine.check(&identity, perm), "Admin missing {:?}", perm);
        }
    }

    #[test]
    fn multi_role_union() {
        let engine = RbacEngine::new();
        // 同时拥有 Viewer 和 Admin 角色，权限取并集
        let identity = make_identity(vec![Role::Viewer, Role::Admin]);
        assert!(engine.check(&identity, &Permission::SessionView));
        assert!(engine.check(&identity, &Permission::AdminSettings));
        assert!(engine.check(&identity, &Permission::WorkerManage));
    }

    #[test]
    fn has_role_check() {
        let identity = make_identity(vec![Role::Operator]);
        assert!(RbacEngine::has_role(&identity, &Role::Operator));
        assert!(!RbacEngine::has_role(&identity, &Role::Admin));
    }

    #[test]
    fn get_permissions_returns_correct_set() {
        let engine = RbacEngine::new();
        let viewer_perms = engine.get_permissions(&Role::Viewer);
        assert_eq!(viewer_perms.len(), 3);
    }
}
