/// Cindy's locale files, as `scripts/sync-locales.sh` copied them.
pub const CATALOGS: [(&str, &str, &str); 12] = [
    (
        "zh-CN",
        include_str!("../../locales/zh-CN/common.json"),
        include_str!("../../locales/zh-CN/aiRename.json"),
    ),
    (
        "zh-TW",
        include_str!("../../locales/zh-TW/common.json"),
        include_str!("../../locales/zh-TW/aiRename.json"),
    ),
    (
        "en",
        include_str!("../../locales/en/common.json"),
        include_str!("../../locales/en/aiRename.json"),
    ),
    (
        "ja",
        include_str!("../../locales/ja/common.json"),
        include_str!("../../locales/ja/aiRename.json"),
    ),
    (
        "ko",
        include_str!("../../locales/ko/common.json"),
        include_str!("../../locales/ko/aiRename.json"),
    ),
    (
        "es",
        include_str!("../../locales/es/common.json"),
        include_str!("../../locales/es/aiRename.json"),
    ),
    (
        "fr",
        include_str!("../../locales/fr/common.json"),
        include_str!("../../locales/fr/aiRename.json"),
    ),
    (
        "de",
        include_str!("../../locales/de/common.json"),
        include_str!("../../locales/de/aiRename.json"),
    ),
    (
        "pt-PT",
        include_str!("../../locales/pt-PT/common.json"),
        include_str!("../../locales/pt-PT/aiRename.json"),
    ),
    (
        "pt-BR",
        include_str!("../../locales/pt-BR/common.json"),
        include_str!("../../locales/pt-BR/aiRename.json"),
    ),
    (
        "zh-Hant-x-lzh",
        include_str!("../../locales/zh-Hant-x-lzh/common.json"),
        include_str!("../../locales/zh-Hant-x-lzh/aiRename.json"),
    ),
    (
        "zh-Hans-x-lzh",
        include_str!("../../locales/zh-Hans-x-lzh/common.json"),
        include_str!("../../locales/zh-Hans-x-lzh/aiRename.json"),
    ),
];

/// Cindy's `DEV_WRITE_BLOCKED`, laid over `chatgptAuthRecovery.devWriteBlocked`.
pub const DEV_WRITE_BLOCKED: [(&str, &str); 7] = [
    (
        "zh-Hant-x-lzh",
        "開發之境，例以唯讀共用此機之 ChatGPT 登入，不得發起登入，亦不得改寫憑證。欲試 OAuth，唯執行：pnpm restart:desktop:remote -- --isolated-auth --isolated。指令碼驗明獨立沙箱後，自會開放寫入，毋須手設環境變數。",
    ),
    (
        "zh-Hans-x-lzh",
        "开发之境，例以只读共用此机之 ChatGPT 登录，不得发起登录，亦不得改写凭证。欲试 OAuth，唯执行：pnpm restart:desktop:remote -- --isolated-auth --isolated。脚本验明独立沙箱后，自会开放写入，毋须手设环境变量。",
    ),
    (
        "en",
        "Development builds use this computer’s ChatGPT sign-in read-only by default and cannot start OAuth or change its credentials. To test OAuth, run: pnpm restart:desktop:remote -- --isolated-auth --isolated. The restart script enables writes only after verifying the sandbox.",
    ),
    (
        "zh-CN",
        "开发环境默认只读共享本机 ChatGPT 登录，不能发起登录或改写凭证。测试 OAuth 只能运行：pnpm restart:desktop:remote -- --isolated-auth --isolated。脚本验证独立沙箱后会自动开放写入，无需手动设置环境变量。",
    ),
    (
        "zh-TW",
        "開發環境預設以唯讀方式共用本機 ChatGPT 登入，不能發起登入或改寫憑證。測試 OAuth 只能執行：pnpm restart:desktop:remote -- --isolated-auth --isolated。指令碼驗證獨立沙箱後會自動開放寫入，無需手動設定環境變數。",
    ),
    (
        "ja",
        "開発環境では、このコンピューターの ChatGPT ログインを既定で読み取り専用として共有するため、OAuth の開始や認証情報の変更はできません。OAuth をテストする場合は、pnpm restart:desktop:remote -- --isolated-auth --isolated を実行してください。スクリプトはサンドボックスを検証した後にのみ書き込みを許可します。環境変数を手動で設定する必要はありません。",
    ),
    (
        "ko",
        "개발 환경은 기본적으로 이 컴퓨터의 ChatGPT 로그인을 읽기 전용으로 공유하므로 OAuth를 시작하거나 자격 증명을 변경할 수 없습니다. OAuth를 테스트하려면 pnpm restart:desktop:remote -- --isolated-auth --isolated을 실행하세요. 스크립트가 샌드박스를 검증한 뒤에만 쓰기를 허용하므로 환경 변수를 직접 설정할 필요가 없습니다.",
    ),
];
