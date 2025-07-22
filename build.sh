#!/bin/bash

# Xbox手柄控制器项目构建脚本
# 提供常用的项目管理命令

set -e  # 遇到错误时退出

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 项目信息
PROJECT_NAME="Xbox手柄控制器"
VERSION="0.1.0"

# 打印带颜色的消息
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 显示帮助信息
show_help() {
    echo -e "${BLUE}${PROJECT_NAME} v${VERSION} 构建脚本${NC}"
    echo ""
    echo "用法: $0 [命令]"
    echo ""
    echo "可用命令:"
    echo "  help          显示此帮助信息"
    echo "  check         检查代码编译状态"
    echo "  test          运行所有测试"
    echo "  build         构建debug版本"
    echo "  release       构建release版本"
    echo "  clean         清理构建文件"
    echo "  lint          代码格式检查"
    echo "  format        格式化代码"
    echo "  install       安装到系统（需要sudo权限）"
    echo "  uninstall     从系统卸载（需要sudo权限）"
    echo "  config        创建默认配置文件"
    echo "  deps          检查系统依赖"
    echo ""
}

# 检查是否在项目根目录
check_project_root() {
    if [[ ! -f "Cargo.toml" ]]; then
        print_error "请在项目根目录运行此脚本"
        exit 1
    fi
}

# 检查系统依赖
check_dependencies() {
    print_info "检查系统依赖..."

    # 检查Rust工具链
    if ! command -v cargo &> /dev/null; then
        print_error "未找到cargo，请安装Rust工具链"
        print_info "安装命令: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi

    # 检查系统库
    case "$(uname -s)" in
        Darwin)
            print_info "检测到macOS系统"
            # 检查Xcode命令行工具
            if ! xcode-select -p &> /dev/null; then
                print_warning "未检测到Xcode命令行工具，某些功能可能无法正常工作"
                print_info "安装命令: xcode-select --install"
            fi
            ;;
        Linux)
            print_info "检测到Linux系统"
            # 检查必要的系统库
            if ! pkg-config --exists libudev; then
                print_warning "未找到libudev开发库，请安装相应的包"
                print_info "Ubuntu/Debian: sudo apt-get install libudev-dev"
                print_info "CentOS/RHEL: sudo yum install systemd-devel"
            fi
            ;;
        *)
            print_warning "未知操作系统，可能需要额外配置"
            ;;
    esac

    print_success "依赖检查完成"
}

# 代码检查
check_code() {
    print_info "检查代码编译状态..."
    cargo check
    print_success "代码检查完成"
}

# 运行测试
run_tests() {
    print_info "运行单元测试..."
    cargo test
    print_success "所有测试通过"
}

# 构建debug版本
build_debug() {
    print_info "构建debug版本..."
    cargo build
    print_success "Debug版本构建完成"
    print_info "可执行文件位置: target/debug/controller"
}

# 构建release版本
build_release() {
    print_info "构建release版本..."
    cargo build --release
    print_success "Release版本构建完成"
    print_info "可执行文件位置: target/release/controller"

    # 显示二进制文件信息
    if [[ -f "target/release/controller" ]]; then
        local size=$(du -h target/release/controller | cut -f1)
        print_info "二进制文件大小: ${size}"
    fi
}

# 清理构建文件
clean_build() {
    print_info "清理构建文件..."
    cargo clean
    print_success "构建文件已清理"
}

# 代码格式检查
lint_code() {
    print_info "运行代码格式检查..."

    # 检查是否安装了clippy
    if ! cargo clippy --version &> /dev/null; then
        print_warning "未安装clippy，正在安装..."
        rustup component add clippy
    fi

    cargo clippy -- -D warnings
    print_success "代码格式检查通过"
}

# 格式化代码
format_code() {
    print_info "格式化代码..."

    # 检查是否安装了rustfmt
    if ! cargo fmt --version &> /dev/null; then
        print_warning "未安装rustfmt，正在安装..."
        rustup component add rustfmt
    fi

    cargo fmt
    print_success "代码格式化完成"
}

# 安装到系统
install_system() {
    print_info "安装到系统..."

    # 确保release版本已构建
    if [[ ! -f "target/release/controller" ]]; then
        print_info "未找到release版本，正在构建..."
        build_release
    fi

    # 安装二进制文件
    local install_path="/usr/local/bin/xbox-controller"
    print_info "复制到 ${install_path}..."
    sudo cp target/release/controller "${install_path}"
    sudo chmod +x "${install_path}"

    print_success "安装完成！"
    print_info "使用命令 'xbox-controller' 启动程序"
}

# 从系统卸载
uninstall_system() {
    print_info "从系统卸载..."

    local install_path="/usr/local/bin/xbox-controller"
    if [[ -f "${install_path}" ]]; then
        sudo rm "${install_path}"
        print_success "卸载完成"
    else
        print_warning "未找到已安装的程序"
    fi
}

# 创建配置文件
create_config() {
    print_info "创建默认配置文件..."

    local config_dir="$HOME/.config/controller"
    local config_file="$config_dir/config.toml"

    # 创建配置目录
    mkdir -p "$config_dir"

    # 检查配置文件是否已存在
    if [[ -f "$config_file" ]]; then
        print_warning "配置文件已存在: $config_file"
        read -p "是否覆盖现有配置？[y/N] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            print_info "跳过配置文件创建"
            return
        fi
    fi

    # 复制示例配置文件
    if [[ -f "config.example.toml" ]]; then
        cp config.example.toml "$config_file"
        print_success "配置文件已创建: $config_file"
        print_info "你可以编辑此文件来自定义设置"
    else
        print_error "未找到示例配置文件"
        exit 1
    fi
}

# 显示项目状态
show_status() {
    print_info "项目状态:"
    echo ""

    # Rust版本
    echo "Rust版本: $(rustc --version)"
    echo "Cargo版本: $(cargo --version)"
    echo ""

    # 项目信息
    echo "项目名称: ${PROJECT_NAME}"
    echo "版本: ${VERSION}"
    echo ""

    # 构建状态
    if [[ -f "target/debug/controller" ]]; then
        echo "Debug构建: ✓"
    else
        echo "Debug构建: ✗"
    fi

    if [[ -f "target/release/controller" ]]; then
        echo "Release构建: ✓"
    else
        echo "Release构建: ✗"
    fi

    # 配置文件状态
    local config_file="$HOME/.config/controller/config.toml"
    if [[ -f "$config_file" ]]; then
        echo "配置文件: ✓ ($config_file)"
    else
        echo "配置文件: ✗ (使用默认配置)"
    fi

    # 系统安装状态
    if [[ -f "/usr/local/bin/xbox-controller" ]]; then
        echo "系统安装: ✓"
    else
        echo "系统安装: ✗"
    fi
}

# 主函数
main() {
    # 检查项目根目录
    check_project_root

    # 解析命令行参数
    case "${1:-help}" in
        help|--help|-h)
            show_help
            ;;
        check)
            check_code
            ;;
        test)
            run_tests
            ;;
        build)
            build_debug
            ;;
        release)
            build_release
            ;;
        clean)
            clean_build
            ;;
        lint)
            lint_code
            ;;
        format)
            format_code
            ;;
        install)
            install_system
            ;;
        uninstall)
            uninstall_system
            ;;
        config)
            create_config
            ;;
        deps)
            check_dependencies
            ;;
        status)
            show_status
            ;;
        all)
            print_info "执行完整构建流程..."
            check_dependencies
            lint_code
            format_code
            run_tests
            build_release
            print_success "完整构建流程完成！"
            ;;
        *)
            print_error "未知命令: $1"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# 运行主函数
main "$@"
