#include <algorithm>
#include <cstddef>
template <std::size_t N>
struct fixed_string {
  constexpr fixed_string(const char (&foo)[N + 1]) {
    std::copy_n(foo, N + 1, data);
  }
  auto operator<=>(const fixed_string&) const = default;
  char data[N + 1] = {};
};
template <std::size_t N>
fixed_string(const char (&str)[N]) -> fixed_string<N - 1>;

template <fixed_string Tag, typename T>
struct tag_and_value {
  T value;
};

template <typename... TagsAndValues>
struct parms : TagsAndValues... {};

template <typename... TagsAndValues>
parms(TagsAndValues...) -> parms<TagsAndValues...>;

template <fixed_string Tag>
struct arg_type {
  template <typename T>
  constexpr auto operator=(T t) const {
    return tag_and_value<Tag, T>{std::move(t)};
  }
};

template <fixed_string Tag>
inline constexpr auto arg = arg_type<Tag>{};

template <fixed_string Tag, typename T>
struct member {
  constexpr static auto tag() { return Tag; }
  using element_type = T;
  T value;
  template <typename OtherT>
  constexpr member(tag_and_value<Tag, OtherT> tv) : value(std::move(tv.value)) {}

  constexpr member() = default;
  constexpr member(member&&) = default;
  constexpr member(const member&) = default;

  constexpr member& operator=(member&&) = default;
  constexpr member& operator=(const member&) = default;

  constexpr auto operator<=>(const member&) const = default;
};

template <typename... Members>
struct meta_struct_impl : Members... {
  template <typename Parms>
  constexpr meta_struct_impl(Parms p)
      : Members(std::move(p))... {}

  constexpr meta_struct_impl() = default;
  constexpr meta_struct_impl(meta_struct_impl&&) = default;
  constexpr meta_struct_impl(const meta_struct_impl&) = default;
  constexpr meta_struct_impl& operator=(meta_struct_impl&&) = default;
  constexpr meta_struct_impl& operator=(const meta_struct_impl&) = default;

  constexpr auto operator<=>(const meta_struct_impl&) const = default;
};

template <typename... Members>
struct meta_struct : meta_struct_impl<Members...> {
    using super = meta_struct_impl<Members...>;
  template <typename... TagsAndValues>
  constexpr meta_struct(TagsAndValues... tags_and_values)
      : super(parms(std::move(tags_and_values)...)) {}

  constexpr meta_struct() = default;
  constexpr meta_struct(meta_struct&&) = default;
  constexpr meta_struct(const meta_struct&) = default;
  constexpr meta_struct& operator=(meta_struct&&) = default;
  constexpr meta_struct& operator=(const meta_struct&) = default;

  constexpr auto operator<=>(const meta_struct&) const = default;
};

template <fixed_string tag, typename T>
decltype(auto) get_impl(member<tag, T>& m) {
  return (m.value);
}

template <fixed_string tag, typename T>
decltype(auto) get_impl(const member<tag, T>& m) {
  return (m.value);
}

template <fixed_string tag, typename T>
decltype(auto) get_impl(member<tag, T>&& m) {
  return std::move(m.value);
}

template <fixed_string tag, typename MetaStruct>
decltype(auto) get(MetaStruct&& s) {
  return get_impl<tag>(std::forward<MetaStruct>(s));
}

#include <iostream>
#include <string>

int main() {
  using Person = meta_struct<      //
      member<"id", int>,           //
      member<"name", std::string>  //
      >;

  Person p{arg<"id"> = 1, arg<"name"> = "John"};

  std::cout << get<"id">(p) << " " << get<"name">(p) << "\n";
  p = Person{arg<"name"> = "John", arg<"id"> = 1};
  std::cout << get<"id">(p) << " " << get<"name">(p) << "\n";
}
